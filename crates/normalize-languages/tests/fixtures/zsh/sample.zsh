#!/usr/bin/env zsh

source ./utils.zsh
. helpers.zsh

function greet {
    local name=$1
    echo "Hello, $name"
}

function classify {
    local n=$1
    if [[ $n -lt 0 ]]; then
        echo "negative"
    elif [[ $n -eq 0 ]]; then
        echo "zero"
    else
        echo "positive"
    fi
}

function sum_array {
    local total=0
    for i in "$@"; do
        total=$((total + i))
    done
    echo $total
}

greet "world"
classify 42
sum_array 1 2 3
