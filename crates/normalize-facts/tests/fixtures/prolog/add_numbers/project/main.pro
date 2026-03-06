:- use_module(math_utils).

compute(Op, A, B, Result) :-
    ( Op = add ->
        add(A, B, Result)
    ; Op = multiply ->
        multiply(A, B, Result)
    ).

main :-
    compute(add, 2, 3, Sum),
    write(Sum), nl,
    compute(multiply, 4, 5, Product),
    write(Product), nl.

:- initialization(main).
