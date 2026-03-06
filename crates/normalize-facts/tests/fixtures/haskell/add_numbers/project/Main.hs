module Main where

import MathUtils (add, multiply)

main :: IO ()
main = do
    print (add 2 3)
    print (multiply 4 5)
