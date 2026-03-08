:- module(sample, [factorial/2, member/2, classify/2, parent/2]).

:- use_module(library(lists)).
:- use_module(library(apply)).

% Facts: family relationships
parent(tom, bob).
parent(tom, liz).
parent(bob, ann).
parent(bob, pat).

% Rule: ancestor/2
ancestor(X, Y) :- parent(X, Y).
ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).

% Rule: factorial/2
factorial(0, 1) :- !.
factorial(N, F) :-
    N > 0,
    N1 is N - 1,
    factorial(N1, F1),
    F is N * F1.

% Rule: classify a number
classify(N, negative) :- N < 0, !.
classify(0, zero) :- !.
classify(_, positive).

% Rule: member/2 (list membership)
member(X, [X|_]).
member(X, [_|T]) :- member(X, T).

% Rule: sum of a list
sum_list([], 0).
sum_list([H|T], Sum) :-
    sum_list(T, Rest),
    Sum is H + Rest.

% Rule: max of two numbers
max_val(X, Y, X) :- X >= Y, !.
max_val(_, Y, Y).

% Rule: append two lists
my_append([], L, L).
my_append([H|T], L, [H|R]) :- my_append(T, L, R).
