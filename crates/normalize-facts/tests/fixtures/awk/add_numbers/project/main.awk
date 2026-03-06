@include "math_utils.awk"

BEGIN {
    a = 2
    b = 3
    print "Sum:", add(a, b)
    print "Product:", multiply(a, b)
    print "Square:", square(a)
}
