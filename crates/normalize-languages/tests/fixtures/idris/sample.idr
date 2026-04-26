module Main

import Data.List
import Data.String

data Shape = Circle Double
           | Rectangle Double Double
           | Triangle Double Double Double

record Point where
  constructor MkPoint
  x : Double
  y : Double

||| Compute Euclidean distance between two points
distance : Point -> Point -> Double
distance p1 p2 =
  let dx = p2.x - p1.x
      dy = p2.y - p1.y
  in sqrt (dx * dx + dy * dy)

area : Shape -> Double
area (Circle r) = pi * r * r
area (Rectangle w h) = w * h
area (Triangle a b c) =
  let s = (a + b + c) / 2
  in sqrt (s * (s - a) * (s - b) * (s - c))

classify : Int -> String
classify n =
  if n < 0
    then "negative"
    else if n == 0
      then "zero"
      else "positive"

sumEvens : List Int -> Int
sumEvens [] = 0
sumEvens (x :: xs) =
  if x `mod` 2 == 0
    then x + sumEvens xs
    else sumEvens xs

main : IO ()
main = do
  let p1 = MkPoint 3.0 4.0
  let p2 = MkPoint 0.0 0.0
  printLn (distance p1 p2)
  printLn (area (Circle 5.0))
  printLn (classify (-3))
  printLn (sumEvens [1, 2, 3, 4, 5, 6])
