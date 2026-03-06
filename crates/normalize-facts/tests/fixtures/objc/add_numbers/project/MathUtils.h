#ifndef MATH_UTILS_H
#define MATH_UTILS_H

@interface MathUtils : NSObject

- (int)addA:(int)a b:(int)b;
- (int)multiplyA:(int)a b:(int)b;

@end

typedef struct {
    int history[100];
    int count;
} Calculator;

int add(int a, int b);
int multiply(int a, int b);

#endif
