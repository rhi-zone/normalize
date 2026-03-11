package main

import "sync"

// Correct: mutex passed as pointer
func unlock(mu *sync.Mutex) {
	mu.Unlock()
}

// Correct: RWMutex as pointer
func readUnlock(mu *sync.RWMutex) {
	mu.RUnlock()
}

// Correct: mutex embedded in a struct, struct passed by pointer
type SafeCounter struct {
	mu    sync.Mutex
	count int
}

func (c *SafeCounter) Inc() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.count++
}

// Correct: returns a pointer to Mutex
func newMutex() *sync.Mutex {
	return &sync.Mutex{}
}
