library(stats)
library(utils)

# Classify a number as negative, zero, or positive
classify <- function(n) {
  if (n < 0) {
    return("negative")
  } else if (n == 0) {
    return("zero")
  } else {
    return("positive")
  }
}

# Sum even numbers in a vector
sum_evens <- function(values) {
  total <- 0
  for (v in values) {
    if (v %% 2 == 0) {
      total <- total + v
    }
  }
  return(total)
}

# Count occurrences of each unique value
count_occurrences <- function(values) {
  counts <- list()
  for (v in values) {
    key <- as.character(v)
    if (is.null(counts[[key]])) {
      counts[[key]] <- 1
    } else {
      counts[[key]] <- counts[[key]] + 1
    }
  }
  return(counts)
}

# Compute factorial recursively
factorial_r <- function(n) {
  if (n <= 1) {
    return(1)
  }
  return(n * factorial_r(n - 1))
}

print(classify(-3))
print(sum_evens(1:10))
print(factorial_r(5))
