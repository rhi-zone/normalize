module MathTools

open System
open System.Collections.Generic

// Type definition: a discriminated union
type Shape =
    | Circle of radius: float
    | Rectangle of width: float * height: float
    | Triangle of base_: float * height: float

// Record type
type Point = { X: float; Y: float }

// Compute area of a shape
let area shape =
    match shape with
    | Circle r -> Math.PI * r * r
    | Rectangle(w, h) -> w * h
    | Triangle(b, h) -> 0.5 * b * h

// Classify a number
let classify n =
    if n < 0 then "negative"
    elif n = 0 then "zero"
    else "positive"

// Sum even numbers in a list
let sumEvens values =
    let mutable total = 0
    for v in values do
        if v % 2 = 0 then
            total <- total + v
    total

// Compute factorial recursively
let rec factorial n =
    if n <= 1 then 1
    else n * factorial (n - 1)

// Distance between two points
let distance (a: Point) (b: Point) =
    let dx = b.X - a.X
    let dy = b.Y - a.Y
    Math.Sqrt(dx * dx + dy * dy)

[<EntryPoint>]
let main _ =
    printfn "%s" (classify -5)
    printfn "%d" (sumEvens [1..10])
    printfn "%d" (factorial 5)
    0
