package main

func earlyReturn(x int) int {
	if x < 0 {
		return -1
	}
	if x == 0 {
		return 0
	}
	return x * 2
}
