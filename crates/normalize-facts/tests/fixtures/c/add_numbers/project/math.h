#ifndef MATH_H
#define MATH_H

typedef struct {
    int history[100];
    int count;
} Calculator;

int add(int a, int b);
int multiply(int a, int b);
void calculator_init(Calculator *calc);
int calculator_compute(Calculator *calc, const char *op, int a, int b);

#endif
