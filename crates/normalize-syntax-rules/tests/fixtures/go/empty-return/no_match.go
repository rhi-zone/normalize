package main

import "fmt"

// Early return in the middle — necessary (exits early)
func maybeWork(do bool) {
	if !do {
		return
	}
	fmt.Println("working")
}

// Return with value — necessary
func add(a, b int) int {
	return a + b
}

// Named return — bare return is idiomatic here, but function has result type
func divide(a, b float64) (result float64, err error) {
	if b == 0 {
		err = fmt.Errorf("division by zero")
		return
	}
	result = a / b
	return
}

// Function with return type — return statement with value is required
func greet(name string) string {
	return "Hello, " + name
}
