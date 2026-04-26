import java.util.LinkedList
import kotlin.math.abs

data class Point(val x: Double, val y: Double) {
    fun distanceTo(other: Point): Double {
        val dx = x - other.x
        val dy = y - other.y
        return Math.sqrt(dx * dx + dy * dy)
    }
}

class Queue<T> {
    private val items = LinkedList<T>()

    fun enqueue(item: T) {
        items.addLast(item)
    }

    fun dequeue(): T? {
        return if (items.isEmpty()) null else items.removeFirst()
    }

    fun peek(): T? = items.peekFirst()

    val size: Int get() = items.size
}

// Classify a number
@JvmStatic
fun classify(n: Int): String {
    return when {
        n < 0 -> "negative"
        n == 0 -> "zero"
        else -> "positive"
    }
}

fun sumEvens(numbers: List<Int>): Int {
    var total = 0
    for (n in numbers) {
        if (n % 2 == 0) {
            total += n
        }
    }
    return total
}

fun main() {
    val q = Queue<Int>()
    q.enqueue(1)
    q.enqueue(2)
    println(q.dequeue())
    println(classify(-3))
    println(sumEvens(listOf(1, 2, 3, 4, 5)))
    val p1 = Point(0.0, 0.0)
    val p2 = Point(3.0, 4.0)
    println(p1.distanceTo(p2))
}
