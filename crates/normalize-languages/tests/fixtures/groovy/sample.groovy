import groovy.transform.Immutable
import java.util.ArrayList

@Immutable
class Point {
    int x
    int y

    double distanceTo(Point other) {
        Math.sqrt(Math.pow(other.x - x, 2) + Math.pow(other.y - y, 2))
    }
}

class MathUtils {
    static String classify(int n) {
        if (n < 0) {
            return "negative"
        } else if (n == 0) {
            return "zero"
        } else {
            return "positive"
        }
    }

    static int sumEvens(List<Integer> numbers) {
        return numbers.findAll { it % 2 == 0 }.sum() ?: 0
    }

    static int factorial(int n) {
        if (n <= 1) return 1
        return n * factorial(n - 1)
    }
}

def greet(String name) {
    println "Hello, ${name}!"
}

def numbers = [1, 2, 3, 4, 5]
greet("World")
println MathUtils.classify(-5)
println MathUtils.sumEvens(numbers)
println MathUtils.factorial(5)
