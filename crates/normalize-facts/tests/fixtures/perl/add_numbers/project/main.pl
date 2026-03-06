use strict;
use warnings;
use MathUtils qw(add multiply);

sub main {
    my $sum = add(2, 3);
    my $product = multiply(4, 5);
    print "Sum: $sum\n";
    print "Product: $product\n";
}

main();
