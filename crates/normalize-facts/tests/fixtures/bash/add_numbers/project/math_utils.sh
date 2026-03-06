#!/usr/bin/env bash

add() {
    echo $(( $1 + $2 ))
}

multiply() {
    echo $(( $1 * $2 ))
}

compute() {
    local op="$1"
    local a="$2"
    local b="$3"
    if [ "$op" = "add" ]; then
        add "$a" "$b"
    else
        multiply "$a" "$b"
    fi
}
