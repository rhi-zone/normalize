#include "math.h"
#include <string.h>

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}

void calculator_init(Calculator *calc) {
    calc->count = 0;
}

int calculator_compute(Calculator *calc, const char *op, int a, int b) {
    int result;
    if (strcmp(op, "add") == 0) {
        result = add(a, b);
    } else {
        result = multiply(a, b);
    }
    calc->history[calc->count++] = result;
    return result;
}
