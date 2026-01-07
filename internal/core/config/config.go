// Package config provides configuration management for TrapperKeeper services.
package config

import (
	"encoding/base64"
	"fmt"
	"os"
	"strings"
	"time"
)

// SensorAPIConfig holds configuration for the gRPC sensor API service.
type SensorAPIConfig struct {
	Host           string
	Port           int
	MaxConnections int
	RequestTimeout time.Duration
	MaxBatchSize   int
	DataDir        string
}

// DefaultSensorAPIConfig returns configuration with default values.
func DefaultSensorAPIConfig() *SensorAPIConfig {
	return &SensorAPIConfig{
		Host:           "0.0.0.0",
		Port:           50051,
		MaxConnections: 1000,
		RequestTimeout: 30 * time.Second,
		MaxBatchSize:   1000,
		DataDir:        "./data",
	}
}

// HMACSecrets extracts HMAC secrets from environment variables.
// Supports TK_HMAC_SECRET (single) and TK_HMAC_SECRET_N (rotation).
// Returns map of secret_id -> decoded secret bytes.
// Secret IDs are UUIDv7 (32 hex chars without hyphens) matching API key format.
func HMACSecrets() (map[string][]byte, error) {
	secrets := make(map[string][]byte)

	// Check single secret TK_HMAC_SECRET
	// Format: <secret_id>:<base64_secret>
	if val := os.Getenv("TK_HMAC_SECRET"); val != "" {
		secretID, decoded, err := ParseHMACSecretWithID(val)
		if err != nil {
			return nil, fmt.Errorf("TK_HMAC_SECRET: %w", err)
		}
		if _, exists := secrets[secretID]; exists {
			return nil, fmt.Errorf("duplicate secret_id '%s' found in environment variables (check TK_HMAC_SECRET and TK_HMAC_SECRET_* for conflicts)", secretID)
		}
		secrets[secretID] = decoded
	}

	// Check numbered secrets TK_HMAC_SECRET_1, TK_HMAC_SECRET_2, etc.
	// Multiple secrets enable rotation: old and new keys valid during migration
	for i := 1; ; i++ {
		key := fmt.Sprintf("TK_HMAC_SECRET_%d", i)
		val := os.Getenv(key)
		if val == "" {
			break
		}
		secretID, decoded, err := ParseHMACSecretWithID(val)
		if err != nil {
			return nil, fmt.Errorf("%s: %w", key, err)
		}
		if _, exists := secrets[secretID]; exists {
			return nil, fmt.Errorf("duplicate secret_id '%s' found in environment variables (check TK_HMAC_SECRET and TK_HMAC_SECRET_* for conflicts)", secretID)
		}
		secrets[secretID] = decoded
	}

	return secrets, nil
}

// ParseHMACSecret decodes base64-encoded HMAC secret from environment variable.
func ParseHMACSecret(envValue string) ([]byte, error) {
	decoded, err := base64.StdEncoding.DecodeString(strings.TrimSpace(envValue))
	if err != nil {
		return nil, fmt.Errorf("invalid base64 encoding: %w", err)
	}
	if len(decoded) < 32 {
		return nil, fmt.Errorf("secret must be at least 32 bytes, got %d", len(decoded))
	}
	return decoded, nil
}

// ParseHMACSecretWithID parses secret_id:base64_secret format.
// Secret ID must be 32 hex chars (UUIDv7 without hyphens).
func ParseHMACSecretWithID(envValue string) (secretID string, secret []byte, err error) {
	parts := strings.SplitN(strings.TrimSpace(envValue), ":", 2)
	if len(parts) != 2 {
		return "", nil, fmt.Errorf("format must be <secret_id>:<base64_secret>")
	}

	secretID = parts[0]
	if len(secretID) != 32 {
		return "", nil, fmt.Errorf("secret_id must be 32 hex chars (UUIDv7 without hyphens)")
	}

	for _, c := range secretID {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			return "", nil, fmt.Errorf("secret_id must be hex chars only")
		}
	}

	secret, err = base64.StdEncoding.DecodeString(parts[1])
	if err != nil {
		return "", nil, fmt.Errorf("invalid base64 encoding: %w", err)
	}

	if len(secret) < 32 {
		return "", nil, fmt.Errorf("secret must be at least 32 bytes, got %d", len(secret))
	}

	return secretID, secret, nil
}
