#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    int *data;
    int top;
    int capacity;
} Stack;

Stack *stack_new(int capacity) {
    Stack *s = malloc(sizeof(Stack));
    s->data = malloc(sizeof(int) * capacity);
    s->top = -1;
    s->capacity = capacity;
    return s;
}

int stack_push(Stack *s, int value) {
    if (s->top >= s->capacity - 1) {
        return 0;
    }
    s->data[++(s->top)] = value;
    return 1;
}

int stack_pop(Stack *s, int *out) {
    if (s->top < 0) {
        return 0;
    }
    *out = s->data[(s->top)--];
    return 1;
}

void stack_free(Stack *s) {
    free(s->data);
    free(s);
}

const char *classify(int n) {
    if (n < 0) {
        return "negative";
    } else if (n == 0) {
        return "zero";
    } else {
        return "positive";
    }
}

int sum_evens(int *arr, int len) {
    int total = 0;
    for (int i = 0; i < len; i++) {
        if (arr[i] % 2 == 0) {
            total += arr[i];
        }
    }
    return total;
}

int main(void) {
    Stack *s = stack_new(10);
    stack_push(s, 42);
    int val = 0;
    stack_pop(s, &val);
    printf("popped: %d\n", val);
    printf("classify(-5): %s\n", classify(-5));
    int nums[] = {1, 2, 3, 4, 5};
    printf("sum_evens: %d\n", sum_evens(nums, 5));
    stack_free(s);
    return 0;
}
