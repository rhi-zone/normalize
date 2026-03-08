module Main exposing (main)

import Html exposing (Html, div, text)
import Html.Attributes exposing (class)
import List exposing (filter, foldl)

type Shape
    = Circle Float
    | Rectangle Float Float

type alias Point =
    { x : Float
    , y : Float
    }

square : Float -> Float
square n =
    n * n

distance : Point -> Point -> Float
distance p1 p2 =
    let
        dx =
            p2.x - p1.x

        dy =
            p2.y - p1.y
    in
    sqrt (square dx + square dy)

area : Shape -> Float
area shape =
    case shape of
        Circle r ->
            pi * r * r

        Rectangle w h ->
            w * h

classify : Int -> String
classify n =
    if n < 0 then
        "negative"

    else if n == 0 then
        "zero"

    else
        "positive"

sumEvens : List Int -> Int
sumEvens xs =
    foldl (\x acc -> if modBy 2 x == 0 then acc + x else acc) 0 xs

main : Html msg
main =
    div [ class "app" ]
        [ text (classify -3)
        , text (String.fromFloat (distance { x = 3, y = 4 } { x = 0, y = 0 }))
        ]
