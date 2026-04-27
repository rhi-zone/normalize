# fixtures/julia

Julia fixture file for `.scm` query tests.

- `sample.jl` — module `MathTools` importing from `Statistics` and `LinearAlgebra`, defines a `Point` struct, functions `classify`/`sum_evens`/`factorial`/`distance`, and a short-form `square` function; `classify` is annotated with `@inline` to exercise `macrocall_expression` decoration capture.
