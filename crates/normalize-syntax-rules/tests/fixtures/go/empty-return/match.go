package main

import "fmt"

// Bare return at end of void function — unnecessary
func doWork() {
	fmt.Println("working")
	return
}

// Bare return at end of void method — unnecessary
type Worker struct{}

func (w *Worker) Run() {
	fmt.Println("running")
	return
}
