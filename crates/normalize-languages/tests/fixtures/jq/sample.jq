import "lib/utils" as utils;

# Compute the sum of an array of numbers
def sum: reduce .[] as $x (0; . + $x);

# Compute the mean of an array
def mean: (sum / length);

# Flatten nested objects into dot-separated keys
def flatten_keys(prefix):
  if type == "object" then
    to_entries
    | map(
        .key as $k
        | .value
        | flatten_keys(if prefix == "" then $k else "\(prefix).\($k)" end)
      )
    | add // {}
  else
    {(prefix): .}
  end;

# Filter an array, keeping only elements where the predicate is true
def keep_if(pred): map(select(pred));

# Group items by a key function and count each group
def count_by(f):
  group_by(f)
  | map({key: (.[0] | f), count: length})
  | sort_by(.count)
  | reverse;

# Normalize a string: lowercase and trim whitespace
def normalize_str:
  ascii_downcase
  | ltrimstr(" ")
  | rtrimstr(" ");

# Format a record for display
def format_record:
  "\(.name // "unknown") (\(.type // "n/a")): \(.value // "")";

# Main pipeline: process an array of records
.records
| keep_if(.active == true)
| map({
    name: (.name | normalize_str),
    type: .type,
    value: .value
  })
| count_by(.type)
