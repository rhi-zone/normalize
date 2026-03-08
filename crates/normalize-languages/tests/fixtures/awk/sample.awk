#!/usr/bin/awk -f

BEGIN {
    FS = ","
    OFS = "\t"
    total = 0
    count = 0
}

function classify(n) {
    if (n < 0) {
        return "negative"
    } else if (n == 0) {
        return "zero"
    } else {
        return "positive"
    }
}

function max(a, b) {
    return a > b ? a : b
}

function trim(s) {
    gsub(/^[ \t]+|[ \t]+$/, "", s)
    return s
}

function sum_fields(    i, s) {
    s = 0
    for (i = 1; i <= NF; i++) {
        s += $i
    }
    return s
}

/^#/ {
    next
}

NF > 0 {
    line = trim($0)
    val = $1 + 0
    total += val
    count++
    label = classify(val)
    print NR, val, label
}

END {
    if (count > 0) {
        avg = total / count
        print "total:", total
        print "count:", count
        print "avg:", avg
        print "max tested:", max(total, count)
    }
}
