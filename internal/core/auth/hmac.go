package auth

import (
	"crypto/hmac"
	"crypto/sha256"
	"fmt"
	"strings"
)

// ParseAPIKey extracts secret_id and random_data from API key format.
// Format: tk-v1-<secret_id>-<random_data> (102 chars total).
// Returns ErrInvalidKeyFormat if format doesn't match.
func ParseAPIKey(key string) (secretID, randomData string, err error) {
	parts := strings.Split(key, "-")
	if len(parts) != 4 {
		return "", "", ErrInvalidKeyFormat
	}

	if parts[0] != "tk" {
		return "", "", ErrInvalidKeyFormat
	}

	if parts[1] != "v1" {
		return "", "", ErrInvalidKeyFormat
	}

	secretID = parts[2]
	randomData = parts[3]

	// Validate secret_id is 32 hex chars (UUID without hyphens)
	if len(secretID) != 32 {
		return "", "", ErrInvalidKeyFormat
	}

	// Validate random_data is 64 hex chars (256 bits)
	if len(randomData) != 64 {
		return "", "", ErrInvalidKeyFormat
	}

	for _, c := range secretID + randomData {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			return "", "", ErrInvalidKeyFormat
		}
	}

	return secretID, randomData, nil
}

// ComputeHMAC computes HMAC-SHA256 signature of API key using secret.
func ComputeHMAC(secret []byte, apiKey string) []byte {
	h := hmac.New(sha256.New, secret)
	h.Write([]byte(apiKey))
	return h.Sum(nil)
}

// VerifyHMAC verifies HMAC signature using constant-time comparison.
// Constant-time comparison prevents timing attacks.
// Available for testing and reference. Production code uses ComputeHMAC directly in Authenticate.
func VerifyHMAC(expectedHash, computedHash []byte) bool {
	return hmac.Equal(expectedHash, computedHash)
}

// FormatAPIKey constructs API key from components.
// Used during key generation in web UI.
func FormatAPIKey(secretID, randomData string) string {
	return fmt.Sprintf("tk-v1-%s-%s", secretID, randomData)
}
