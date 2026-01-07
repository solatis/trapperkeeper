// Package auth provides HMAC-based API key authentication for gRPC services.
package auth

import (
	"context"
	"database/sql"
	"fmt"
	"strings"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
)

// contextKey is a typed key for context values to avoid collisions.
type contextKey string

// tenantIDKey is the context key for storing authenticated tenant ID.
const tenantIDKey = contextKey("tenant_id")

// Queries interface defines database operations needed for authentication.
// Implemented by *db.Queries to allow query loading via LoadQueries().
type Queries interface {
	Get(name string, dest interface{}, args ...interface{}) error
	Exec(name string, args ...interface{}) (sql.Result, error)
}

// Authenticator validates API keys using HMAC-SHA256 signatures.
// Holds in-memory secret map for O(1) lookup and queries for key verification.
type Authenticator struct {
	secrets map[string][]byte
	queries Queries
}

// NewAuthenticator creates an authenticator with HMAC secrets and query interface.
func NewAuthenticator(secrets map[string][]byte, queries Queries) *Authenticator {
	return &Authenticator{
		secrets: secrets,
		queries: queries,
	}
}

// Authenticate validates API key and returns tenant_id on success.
// Returns specific error for each failure mode (5-tier taxonomy).
func (a *Authenticator) Authenticate(ctx context.Context, apiKey string) (string, error) {
	// Parse API key format
	secretID, _, err := ParseAPIKey(apiKey)
	if err != nil {
		return "", err
	}

	// O(1) lookup of HMAC secret using secret_id from key format
	secret, ok := a.secrets[secretID]
	if !ok {
		return "", ErrUnknownKey
	}

	computedHash := ComputeHMAC(secret, apiKey)

	// Query database by key_hash using named query (unique constraint ensures single result)
	var result struct {
		TenantID   string       `db:"tenant_id"`
		RevokedAt  sql.NullTime `db:"revoked_at"`
		APIKeyID   string       `db:"api_key_id"`
		LastUsedAt sql.NullTime `db:"last_used_at"`
	}

	err = a.queries.Get("get-api-key-by-hash", &result, computedHash)
	if err == sql.ErrNoRows {
		return "", ErrInvalidKey
	}
	if err != nil {
		return "", fmt.Errorf("database error: %w", err)
	}

	// Check revocation status
	if result.RevokedAt.Valid {
		return "", ErrKeyRevoked
	}

	// Update last_used_at if >1 minute since last update
	// 1-minute throttle reduces write amplification by 99%+ for active sensors
	if shouldUpdateLastUsed(result.LastUsedAt) {
		_, _ = a.queries.Exec("update-last-used", time.Now().UTC(), result.APIKeyID)
	}

	return result.TenantID, nil
}

// shouldUpdateLastUsed implements 1-minute throttle to reduce write amplification.
func shouldUpdateLastUsed(lastUsed sql.NullTime) bool {
	if !lastUsed.Valid {
		return true
	}
	return time.Since(lastUsed.Time) > time.Minute
}

// UnaryInterceptor returns gRPC interceptor that authenticates requests.
func (a *Authenticator) UnaryInterceptor() grpc.UnaryServerInterceptor {
	return func(ctx context.Context, req interface{}, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (interface{}, error) {
		md, ok := metadata.FromIncomingContext(ctx)
		if !ok {
			return nil, status.Error(codes.Unauthenticated, "missing metadata")
		}

		apiKeys := md.Get("x-api-key")
		if len(apiKeys) == 0 {
			return nil, status.Error(codes.Unauthenticated, ErrMissingKey.Error())
		}

		tenantID, err := a.Authenticate(ctx, apiKeys[0])
		if err != nil {
			if err == ErrKeyRevoked {
				return nil, status.Error(codes.PermissionDenied, err.Error())
			}
			// Check for database errors - return UNAVAILABLE instead of UNAUTHENTICATED
			if strings.Contains(err.Error(), "database error") {
				return nil, status.Error(codes.Unavailable, err.Error())
			}
			return nil, status.Error(codes.Unauthenticated, err.Error())
		}

		// Inject tenant_id into context for downstream handlers
		ctx = context.WithValue(ctx, tenantIDKey, tenantID)
		return handler(ctx, req)
	}
}

// TenantIDFromContext extracts tenant ID from context.
// Returns empty string if not found.
func TenantIDFromContext(ctx context.Context) string {
	if tenantID, ok := ctx.Value(tenantIDKey).(string); ok {
		return tenantID
	}
	return ""
}
