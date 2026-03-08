#!/usr/bin/env zsh
setopt ERR_EXIT PIPE_FAIL

source ~/.zshrc.d/utils.zsh
. ~/.config/zsh/helpers.zsh

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

greet() {
    local name="${1:-World}"
    echo "Hello, ${name}!"
}

sum_array() {
    local total=0
    local nums=("$@")
    for num in "${nums[@]}"; do
        (( total += num ))
    done
    echo "$total"
}

repeat_msg() {
    local msg="$1"
    local count="$2"
    local i=0
    while (( i < count )); do
        echo "$msg"
        (( i++ ))
    done
}

setup_dir() {
    local dir="${1:-.}"
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
        echo "Created: $dir"
    else
        echo "Exists: $dir"
    fi
}

greet "Zsh"
classify -3
classify 0
classify 5
sum_array 1 2 3 4 5
repeat_msg "hello" 3
setup_dir "/tmp/zsh_test"
