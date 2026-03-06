#import "MathUtils.h"

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}

@implementation MathUtils

- (int)addA:(int)a b:(int)b {
    return add(a, b);
}

- (int)multiplyA:(int)a b:(int)b {
    return multiply(a, b);
}

@end
