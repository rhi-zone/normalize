open Belt
open Belt.Array

type point = {
  x: float,
  y: float,
}

type shape =
  | Circle(float)
  | Rectangle(float, float)

let square = (n: float) => n *. n

let distance = (p1: point, p2: point) => {
  let dx = p2.x -. p1.x
  let dy = p2.y -. p1.y
  Js.Math.sqrt(square(dx) +. square(dy))
}

let area = (s: shape) =>
  switch s {
  | Circle(r) => Js.Math._PI *. r *. r
  | Rectangle(w, h) => w *. h
  }

/** Classify a number as negative, zero, or positive. */
@inline
let classify = (n: int) =>
  if n < 0 {
    "negative"
  } else if n == 0 {
    "zero"
  } else {
    "positive"
  }

let sumEvens = (xs: array<int>) =>
  Array.reduce(xs, 0, (acc, x) =>
    if mod_float(float_of_int(x), 2.0) == 0.0 {
      acc + x
    } else {
      acc
    }
  )

let main = () => {
  let p1 = {x: 3.0, y: 4.0}
  let p2 = {x: 0.0, y: 0.0}
  Js.log(distance(p1, p2))
  Js.log(classify(-3))
  Js.log(sumEvens([1, 2, 3, 4, 5, 6]))
}
