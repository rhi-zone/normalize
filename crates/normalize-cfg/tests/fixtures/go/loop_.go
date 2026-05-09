package main

func loop_(items []int) int {
	result := 0
	for _, item := range items {
		if item == 0 {
			break
		}
		result += item
	}
	return result
}
