#!/usr/bin/env bash

source math_utils.sh

main() {
    local sum
    sum=$(add 2 3)
    echo "Sum: $sum"

    local product
    product=$(multiply 4 5)
    echo "Product: $product"

    local result
    result=$(compute "add" 10 20)
    echo "Result: $result"
}

main
