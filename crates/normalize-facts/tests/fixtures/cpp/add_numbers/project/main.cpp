#include "math_utils.hpp"
#include <iostream>

int main() {
    std::cout << math::add(2, 3) << std::endl;
    std::cout << math::multiply(4, 5) << std::endl;

    math::Calculator calc;
    calc.compute("add", 10, 20);
    std::cout << calc.last_result() << std::endl;

    return 0;
}
