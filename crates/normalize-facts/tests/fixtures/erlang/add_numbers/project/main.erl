-module(main).
-export([start/0]).
-import(math_utils, [add/2]).

start() ->
    Sum = math_utils:add(2, 3),
    Product = math_utils:multiply(4, 5),
    io:format("~p~n~p~n", [Sum, Product]).
