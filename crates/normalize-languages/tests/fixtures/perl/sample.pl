#!/usr/bin/perl
use strict;
use warnings;
use List::Util qw(sum max min);
use POSIX qw(floor ceil);

package Calculator;

sub new {
    my ($class, %args) = @_;
    return bless {
        precision => $args{precision} // 2,
    }, $class;
}

sub add {
    my ($self, $a, $b) = @_;
    return $a + $b;
}

sub multiply {
    my ($self, $a, $b) = @_;
    return $a * $b;
}

package main;

sub classify {
    my ($n) = @_;
    if ($n < 0) {
        return "negative";
    } elsif ($n == 0) {
        return "zero";
    } else {
        return "positive";
    }
}

sub sum_array {
    my @nums = @_;
    my $total = 0;
    for my $n (@nums) {
        $total += $n;
    }
    return $total;
}

sub factorial {
    my ($n) = @_;
    return 1 if $n <= 1;
    my $result = 1;
    for my $i (2 .. $n) {
        $result *= $i;
    }
    return $result;
}

my $calc = Calculator->new(precision => 4);
print "add: ", $calc->add(3, 4), "\n";
print "classify(-5): ", classify(-5), "\n";
print "sum: ", sum_array(1, 2, 3, 4, 5), "\n";
print "factorial(5): ", factorial(5), "\n";
print "max: ", max(1, 7, 3), "\n";
