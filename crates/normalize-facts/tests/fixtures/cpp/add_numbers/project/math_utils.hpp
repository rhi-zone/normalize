#ifndef MATH_UTILS_HPP
#define MATH_UTILS_HPP

namespace math {

int add(int a, int b);
int multiply(int a, int b);

class Calculator {
public:
    Calculator();
    int compute(const char *op, int a, int b);
    int last_result() const;

private:
    int result_;
};

} // namespace math

#endif
