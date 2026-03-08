#!/usr/bin/env fish

source ~/.config/fish/functions/utils.fish

function classify
    set n $argv[1]
    if test $n -lt 0
        echo "negative"
    else if test $n -eq 0
        echo "zero"
    else
        echo "positive"
    end
end

function greet
    set name (test (count $argv) -gt 0; and echo $argv[1]; or echo "World")
    echo "Hello, $name!"
end

function sum_list
    set total 0
    for num in $argv
        set total (math $total + $num)
    end
    echo $total
end

function repeat_msg
    set msg $argv[1]
    set count $argv[2]
    for i in (seq 1 $count)
        echo $msg
    end
end

function setup_dir
    set dir $argv[1]
    if not test -d $dir
        mkdir -p $dir
        echo "Created: $dir"
    else
        echo "Exists: $dir"
    end
end

greet "Fish"
classify -3
classify 0
classify 5
sum_list 1 2 3 4 5
repeat_msg "hello" 3
setup_dir /tmp/fish_test
