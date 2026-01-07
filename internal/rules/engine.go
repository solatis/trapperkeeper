package rules

// Engine provides dependency injection interface for service.
// No-op stub for service dependency injection. Provides no evaluation methods.
type Engine struct{}

// NewEngine creates a new rules engine instance.
func NewEngine() *Engine {
	return &Engine{}
}
