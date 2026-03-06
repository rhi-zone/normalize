package main

import "os"

func handleError() error {
	if err := os.Remove("/tmp/file.txt"); err != nil {
		return err
	}
	result, err := os.Open("/tmp/file.txt")
	if err != nil {
		return err
	}
	_ = result
	return nil
}
