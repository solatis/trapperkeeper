package auth

import "errors"

// Authentication error types enable 5-tier error taxonomy.
// UNAUTHENTICATED for missing/invalid (doesn't confirm key existence).
// PERMISSION_DENIED for revoked (confirms key exists but blocked).
var (
	ErrMissingKey       = errors.New("API key required in x-api-key metadata")
	ErrInvalidKeyFormat = errors.New("invalid API key format")
	ErrUnknownKey       = errors.New("unknown secret ID")
	ErrInvalidKey       = errors.New("invalid API key")
	ErrKeyRevoked       = errors.New("API key has been revoked")
)
