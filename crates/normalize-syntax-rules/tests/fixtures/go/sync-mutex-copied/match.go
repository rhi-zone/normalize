package main

import "sync"

// Mutex passed by value as parameter — copies the mutex state
func unlock(mu sync.Mutex) {
	mu.Unlock()
}

// RWMutex passed by value as parameter
func readUnlock(mu sync.RWMutex) {
	mu.RUnlock()
}

// Function returns Mutex by value — caller receives a copy
func newMutex() sync.Mutex {
	return sync.Mutex{}
}

// Method with Mutex by-value parameter
type Server struct{}

func (s *Server) protect(mu sync.Mutex, fn func()) {
	mu.Lock()
	defer mu.Unlock()
	fn()
}
