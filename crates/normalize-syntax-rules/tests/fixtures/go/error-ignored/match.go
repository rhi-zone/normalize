package main

import "os"

func discardError() {
	_ = os.Remove("/tmp/file.txt")
	_ = os.Mkdir("/tmp/dir", 0o755)
}
