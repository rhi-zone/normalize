module MathTools

import Statistics
using LinearAlgebra: norm, dot

# Classify a number
@inline function classify(n::Int)::String
    if n < 0
        return "negative"
    elseif n == 0
        return "zero"
    else
        return "positive"
    end
end

# Sum even numbers in a vector
function sum_evens(values::Vector{Int})::Int
    total = 0
    for v in values
        if v % 2 == 0
            total += v
        end
    end
    return total
end

# Compute factorial recursively
function factorial(n::Int)::Int
    if n <= 1
        return 1
    end
    return n * factorial(n - 1)
end

# Short-form function: square
square(x) = x * x

# Struct definition
struct Point
    x::Float64
    y::Float64
end

# Method on struct
function distance(a::Point, b::Point)::Float64
    return norm([b.x - a.x, b.y - a.y])
end

end # module MathTools
