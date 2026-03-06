source("math_utils.R")

main <- function() {
    sum_result <- add_numbers(2, 3)
    product <- multiply(4, 5)
    cat("Sum:", sum_result, "\n")
    cat("Product:", product, "\n")
}

main()
