import Mathlib.Data.List.Basic
import Mathlib.Data.Nat.Basic

structure Point where
  x : Float
  y : Float

def distance (p1 p2 : Point) : Float :=
  let dx := p2.x - p1.x
  let dy := p2.y - p1.y
  Float.sqrt (dx * dx + dy * dy)

@[inline]
def classify (n : Int) : String :=
  if n < 0 then "negative"
  else if n == 0 then "zero"
  else "positive"

def sumEvens (xs : List Int) : Int :=
  xs.foldl (fun acc x => if x % 2 == 0 then acc + x else acc) 0

theorem distance_nonneg (p1 p2 : Point) : distance p1 p2 ≥ 0 := by
  simp [distance]
  apply Float.sqrt_nonneg

def circleArea (r : Float) : Float :=
  Float.PI * r * r

inductive Shape where
  | circle : Float -> Shape
  | rectangle : Float -> Float -> Shape

def shapeArea : Shape -> Float
  | Shape.circle r => circleArea r
  | Shape.rectangle w h => w * h

def main : IO Unit := do
  let p1 : Point := { x := 3.0, y := 4.0 }
  let p2 : Point := { x := 0.0, y := 0.0 }
  IO.println s!"distance: {distance p1 p2}"
  IO.println s!"classify: {classify (-3)}"
  IO.println s!"sumEvens: {sumEvens [1, 2, 3, 4, 5, 6]}"
