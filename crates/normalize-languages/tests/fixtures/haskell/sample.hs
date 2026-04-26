{-# LANGUAGE ScopedTypeVariables #-}
module Main where

import Data.List (sort, nub)
import Data.Map (Map)
import qualified Data.Map as Map

-- | A simple data type for a tree
data Tree a = Leaf | Node a (Tree a) (Tree a)

-- | Newtype wrapper for a count
newtype Count = Count Int

-- | Type synonym
type Name = String

-- | Insert a value into a BST
insert :: Ord a => a -> Tree a -> Tree a
insert x Leaf = Node x Leaf Leaf
insert x (Node y left right)
    | x < y    = Node y (insert x left) right
    | x > y    = Node y left (insert x right)
    | otherwise = Node y left right

-- | Check membership in a BST
member :: Ord a => a -> Tree a -> Bool
member _ Leaf = False
member x (Node y left right)
    | x == y = True
    | x < y  = member x left
    | otherwise = member x right

-- | Classify a number
classify :: Int -> String
classify n =
    if n < 0
        then "negative"
        else if n == 0
            then "zero"
            else "positive"

-- | Count unique elements in a list
countUnique :: Ord a => [a] -> Int
countUnique xs = length (nub (sort xs))

-- | Build frequency map
frequencyMap :: Ord a => [a] -> Map a Int
frequencyMap = foldr (\x m -> Map.insertWith (+) x 1 m) Map.empty

main :: IO ()
main = do
    let t = insert 3 (insert 1 (insert 2 Leaf))
    print (member 2 t)
    print (classify (-5))
    print (countUnique [1, 2, 1, 3, 2])
