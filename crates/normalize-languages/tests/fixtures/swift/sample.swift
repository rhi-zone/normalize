import Foundation
import Swift

struct Point {
    let x: Double
    let y: Double

    func distanceTo(_ other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}

class Stack<T> {
    private var items: [T] = []

    func push(_ item: T) {
        items.append(item)
    }

    func pop() -> T? {
        if items.isEmpty {
            return nil
        }
        return items.removeLast()
    }

    func peek() -> T? {
        return items.last
    }

    var isEmpty: Bool {
        return items.isEmpty
    }

    var count: Int {
        return items.count
    }
}

func classify(_ n: Int) -> String {
    if n < 0 {
        return "negative"
    } else if n == 0 {
        return "zero"
    } else {
        return "positive"
    }
}

func sumEvens(_ numbers: [Int]) -> Int {
    var total = 0
    for n in numbers {
        if n % 2 == 0 {
            total += n
        }
    }
    return total
}

let stack = Stack<Int>()
stack.push(10)
stack.push(20)
print(stack.pop() ?? 0)
print(classify(-5))
print(sumEvens([1, 2, 3, 4, 5]))
