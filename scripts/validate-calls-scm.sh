#!/usr/bin/env sh
# Validate capture names in .calls.scm query files and output SARIF 2.1.0.
#
# Valid capture names: @call, @call.write, @call.qualifier
# Any other capture name is flagged as a warning.
#
# Usage:
#   ./scripts/validate-calls-scm.sh [<root>]
#   ./scripts/validate-calls-scm.sh | cat
#
# Exit code: 1 if violations found, 0 otherwise.

set -eu

ROOT="${1:-.}"
VALID="call call.write call.qualifier"

SARIF_SCHEMA="https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json"

# Collect violations as newline-separated "file:line:col:name" records
violations=""

for scm_file in $(find "$ROOT" -name "*.calls.scm" 2>/dev/null | sort); do
    lineno=0
    while IFS= read -r line; do
        lineno=$((lineno + 1))
        # Strip comment: everything from ';' onward
        code="${line%%;*}"
        # Extract @capture_name tokens (word chars + dots)
        remainder="$code"
        col_offset=0
        while true; do
            # Find next '@' in remainder
            before="${remainder%%@*}"
            if [ "$before" = "$remainder" ]; then
                break  # no more '@'
            fi
            after="${remainder#*@}"
            # Extract the capture name: leading word chars and dots
            name=$(printf '%s' "$after" | sed 's/^\([A-Za-z_][A-Za-z0-9_.]*\).*/\1/')
            if [ -n "$name" ]; then
                col=$((col_offset + ${#before} + 1))  # 1-based column of '@' in full line
                valid=0
                for v in $VALID; do
                    if [ "$name" = "$v" ]; then
                        valid=1
                        break
                    fi
                done
                if [ "$valid" -eq 0 ]; then
                    violations="${violations}${scm_file}:${lineno}:${col}:${name}
"
                fi
                col_offset=$((col_offset + ${#before} + 1))
            fi
            remainder="$after"
        done
    done < "$scm_file"
done

# Build SARIF output
TOOL_SECTION='"tool":{"driver":{"name":"normalize-validate-calls-scm","version":"1.0.0","rules":[{"id":"normalize/invalid-calls-capture","name":"InvalidCallsCaptureName","shortDescription":{"text":"Invalid capture name in .calls.scm file"},"defaultConfiguration":{"level":"warning"}}]}}'

printf '{"$schema":"%s","version":"2.1.0","runs":[{%s,"results":[' \
    "$SARIF_SCHEMA" "$TOOL_SECTION"

first=1
while IFS= read -r rec; do
    [ -z "$rec" ] && continue
    # Parse "file:line:col:name" — name may contain dots but not colons
    file=$(printf '%s' "$rec" | cut -d: -f1)
    lineno=$(printf '%s' "$rec" | cut -d: -f2)
    col=$(printf '%s' "$rec" | cut -d: -f3)
    name=$(printf '%s' "$rec" | cut -d: -f4-)

    msg="Unexpected capture name @${name} in .calls.scm file. Valid names are: @call, @call.qualifier, @call.write."

    if [ "$first" -eq 0 ]; then
        printf ','
    fi
    first=0

    printf '{"ruleId":"normalize/invalid-calls-capture","level":"warning","message":{"text":"%s"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"%s"},"region":{"startLine":%s,"startColumn":%s}}}]}' \
        "$msg" "$file" "$lineno" "$col"
done << EOF
$violations
EOF

printf ']}]}'
printf '\n'

# Exit 1 if any violations
[ -z "$(printf '%s' "$violations" | tr -d '\n')" ]
