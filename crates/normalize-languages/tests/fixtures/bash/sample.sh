#!/usr/bin/env bash
set -euo pipefail

source ./utils.sh
. ./config.sh

classify() {
    local n="$1"
    if (( n < 0 )); then
        echo "negative"
    elif (( n == 0 )); then
        echo "zero"
    else
        echo "positive"
    fi
}

sum_array() {
    local total=0
    for num in "$@"; do
        (( total += num ))
    done
    echo "$total"
}

greet() {
    local name="${1:-World}"
    echo "Hello, ${name}!"
}

repeat() {
    local msg="$1"
    local count="$2"
    local i=0
    while (( i < count )); do
        echo "$msg"
        (( i++ ))
    done
}

setup_environment() {
    local dir="${1:-.}"
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
    fi
    echo "Environment ready: $dir"
}

main() {
    greet "Bash"
    classify -3
    classify 0
    classify 5
    sum_array 1 2 3 4 5
    repeat "hello" 3
    setup_environment "/tmp/test_env"
}

main "$@"
