package main

import (
	"fmt"
	"os"
)

// defer outside of a loop — runs when processFile returns
func processFile(path string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close() // correct: deferred in the function that opens the file
	fmt.Println(path)
	return nil
}

// defer inside an immediately-invoked function literal in a loop — correct workaround
func openFilesCorrect(paths []string) error {
	for _, path := range paths {
		err := func() error {
			f, err := os.Open(path)
			if err != nil {
				return err
			}
			defer f.Close() // runs when the anonymous func returns (end of iteration)
			fmt.Println(path)
			return nil
		}()
		if err != nil {
			return err
		}
	}
	return nil
}
