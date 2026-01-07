package api

// Error mapping is done inline in handlers.
// Auth errors mapped in auth package interceptor.
// Database errors map to UNAVAILABLE.
// Validation errors map to INVALID_ARGUMENT.
// Context timeouts map to DEADLINE_EXCEEDED.
