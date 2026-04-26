#include <iostream>
#include <vector>
#include <stdexcept>

template <typename T>
class Stack {
public:
    // Pushes an item onto the stack.
    void push(const T& item) {
        items.push_back(item);
    }

    T pop() {
        if (items.empty()) {
            throw std::underflow_error("Stack is empty");
        }
        T top = items.back();
        items.pop_back();
        return top;
    }

    const T& peek() const {
        if (items.empty()) {
            throw std::underflow_error("Stack is empty");
        }
        return items.back();
    }

    bool empty() const { return items.empty(); }
    std::size_t size() const { return items.size(); }

private:
    std::vector<T> items;
};

std::string classify(int n) {
    if (n < 0) {
        return "negative";
    } else if (n == 0) {
        return "zero";
    } else {
        return "positive";
    }
}

int sum_evens(const std::vector<int>& numbers) {
    int total = 0;
    for (int n : numbers) {
        if (n % 2 == 0) {
            total += n;
        }
    }
    return total;
}

int main() {
    Stack<int> s;
    s.push(10);
    s.push(20);
    std::cout << s.pop() << "\n";
    std::cout << classify(-3) << "\n";
    std::vector<int> nums = {1, 2, 3, 4, 5};
    std::cout << sum_evens(nums) << "\n";
    return 0;
}
