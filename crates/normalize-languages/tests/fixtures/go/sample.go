package main

import (
	"fmt"
	"strings"
)

// Stack is a generic LIFO structure.
type Stack struct {
	items []string
}

func NewStack() *Stack {
	return &Stack{}
}

func (s *Stack) Push(item string) {
	s.items = append(s.items, item)
}

func (s *Stack) Pop() (string, bool) {
	if len(s.items) == 0 {
		return "", false
	}
	last := s.items[len(s.items)-1]
	s.items = s.items[:len(s.items)-1]
	return last, true
}

func Classify(n int) string {
	if n < 0 {
		return "negative"
	} else if n == 0 {
		return "zero"
	}
	return "positive"
}

func JoinWords(words []string, sep string) string {
	result := strings.Join(words, sep)
	fmt.Println(result)
	return result
}
