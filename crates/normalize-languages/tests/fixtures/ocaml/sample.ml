open List
open Printf

(* Module for stack operations *)
module Stack = struct
  type 'a t = { mutable items : 'a list }

  let create () = { items = [] }

  let push s x = s.items <- x :: s.items

  let pop s =
    match s.items with
    | [] -> None
    | x :: rest ->
        s.items <- rest;
        Some x

  let is_empty s = s.items = []
end

(* Type definition for a binary tree *)
type 'a tree =
  | Leaf
  | Node of 'a * 'a tree * 'a tree

(* Insert into BST *)
let rec insert x = function
  | Leaf -> Node (x, Leaf, Leaf)
  | Node (y, left, right) ->
      if x < y then Node (y, insert x left, right)
      else if x > y then Node (y, left, insert x right)
      else Node (y, left, right)

(** Classify a number as negative, zero, or positive. *)
[@inline]
let classify n =
  if n < 0 then "negative"
  else if n = 0 then "zero"
  else "positive"

(* Sum of even numbers in a list *)
let sum_evens lst =
  fold_left (fun acc x -> if x mod 2 = 0 then acc + x else acc) 0 lst

let () =
  let s = Stack.create () in
  Stack.push s 1;
  Stack.push s 2;
  printf "%s\n" (classify 5);
  printf "%d\n" (sum_evens [1; 2; 3; 4; 5])
