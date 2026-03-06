package MathUtils;

use strict;
use warnings;
use Exporter qw(import);

our @EXPORT_OK = qw(add multiply);

sub add {
    my ($a, $b) = @_;
    return $a + $b;
}

sub multiply {
    my ($a, $b) = @_;
    return $a * $b;
}

1;
