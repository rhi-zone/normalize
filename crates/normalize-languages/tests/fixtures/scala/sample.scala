import scala.collection.mutable.ArrayBuffer
import scala.math.abs

case class Point(x: Double, y: Double) {
  def distanceTo(other: Point): Double = {
    val dx = x - other.x
    val dy = y - other.y
    math.sqrt(dx * dx + dy * dy)
  }
}

class Stack[T] {
  private val items = ArrayBuffer.empty[T]

  def push(item: T): Unit = {
    items.append(item)
  }

  def pop(): Option[T] = {
    if (items.isEmpty) None
    else {
      val top = items.last
      items.remove(items.length - 1)
      Some(top)
    }
  }

  def peek(): Option[T] = items.lastOption

  def size: Int = items.length
}

def classify(n: Int): String = {
  if (n < 0) "negative"
  else if (n == 0) "zero"
  else "positive"
}

def sumEvens(numbers: List[Int]): Int = {
  var total = 0
  for (n <- numbers) {
    if (n % 2 == 0) total += n
  }
  total
}

@main def run(): Unit = {
  val stack = new Stack[Int]()
  stack.push(1)
  stack.push(2)
  println(stack.pop())
  println(classify(-3))
  println(sumEvens(List(1, 2, 3, 4, 5)))
  val p1 = Point(0.0, 0.0)
  val p2 = Point(3.0, 4.0)
  println(p1.distanceTo(p2))
}
