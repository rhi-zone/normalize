import gleam/io
import gleam/list
import gleam/int

// Type definition: a custom type
pub type Shape {
  Circle(radius: Float)
  Rectangle(width: Float, height: Float)
}

// Type alias
pub type Name = String

// Constant
pub const max_size = 100

// Classify a number
pub fn classify(n: Int) -> String {
  case n {
    _ if n < 0 -> "negative"
    0 -> "zero"
    _ -> "positive"
  }
}

// Sum even numbers in a list
pub fn sum_evens(values: List(Int)) -> Int {
  values
  |> list.filter(fn(x) { int.remainder(x, 2) == Ok(0) })
  |> list.fold(0, fn(acc, x) { acc + x })
}

// Compute factorial
pub fn factorial(n: Int) -> Int {
  case n {
    0 -> 1
    1 -> 1
    _ -> n * factorial(n - 1)
  }
}

// Greet a person
pub fn greet(name: String) -> String {
  "Hello, " <> name <> "!"
}

pub fn main() {
  io.println(classify(-3))
  io.println(greet("World"))
}
