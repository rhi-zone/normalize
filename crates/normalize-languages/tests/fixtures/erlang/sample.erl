-module(sample).

-import(lists, [foldl/3, filter/2]).

-export([classify/1, sum_evens/1, factorial/1, member/2]).

-record(point, {x :: integer(), y :: integer()}).

-type coordinate() :: {integer(), integer()}.

-spec classify(integer()) -> negative | zero | positive.
%% Classify a number as negative, zero, or positive
classify(N) when N < 0 ->
    negative;
classify(0) ->
    zero;
classify(_N) ->
    positive.

%% Sum the even numbers in a list
sum_evens(List) ->
    Evens = filter(fun(X) -> X rem 2 =:= 0 end, List),
    foldl(fun(X, Acc) -> X + Acc end, 0, Evens).

%% Compute factorial recursively
factorial(0) ->
    1;
factorial(N) when N > 0 ->
    N * factorial(N - 1).

%% Check if an element is in a list
member(_Elem, []) ->
    false;
member(Elem, [Elem | _Rest]) ->
    true;
member(Elem, [_ | Rest]) ->
    member(Elem, Rest).

%% Create a point record
make_point(X, Y) ->
    #point{x = X, y = Y}.
