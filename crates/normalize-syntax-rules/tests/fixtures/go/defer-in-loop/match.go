package main

import (
	"fmt"
	"os"
)

// defer inside a for/range loop — deferred calls stack up until function returns
func openFiles(paths []string) error {
	for _, path := range paths {
		f, err := os.Open(path)
		if err != nil {
			return err
		}
		defer f.Close() // BUG: closes all files when openFiles returns, not per-iteration
		fmt.Println(path)
	}
	return nil
}

// defer inside a traditional for loop
func countDown() {
	for i := 10; i > 0; i-- {
		defer fmt.Println(i) // deferred calls run in reverse order at function exit
	}
}
