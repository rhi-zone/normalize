module Sample where

import Data.List
import Data.Maybe

-- A simple data type
data Shape : Set where
  Circle  : Shape
  Square  : Shape
  Triangle : Shape

-- A record type
record Point : Set where
  field
    x : Int
    y : Int

-- Function: classify a number
classify : Int → String
classify n =
  if n < 0
    then "negative"
    else if n == 0
      then "zero"
      else "positive"

-- Function: area using pattern matching
area : Shape → Int
area Circle   = 314
area Square   = 100
area Triangle = 50

-- Function: apply to list
applyToAll : (Int → Int) → List Int → List Int
applyToAll f [] = []
applyToAll f (x ∷ xs) = f x ∷ applyToAll f xs

-- Function: double a number
double : Int → Int
double n = n + n

-- Function: sum a list
sumList : List Int → Int
sumList [] = 0
sumList (x ∷ xs) = x + sumList xs
