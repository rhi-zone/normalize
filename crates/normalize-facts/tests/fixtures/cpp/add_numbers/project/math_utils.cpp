#include "math_utils.hpp"
#include <cstring>

namespace math {

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}

Calculator::Calculator() : result_(0) {}

int Calculator::compute(const char *op, int a, int b) {
    if (std::strcmp(op, "add") == 0) {
        result_ = add(a, b);
    } else {
        result_ = multiply(a, b);
    }
    return result_;
}

int Calculator::last_result() const {
    return result_;
}

} // namespace math
