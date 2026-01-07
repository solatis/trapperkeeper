# internal/core/auth/

HMAC-based API key authentication for gRPC and HTTP services.

## Index

| File         | Contents (WHAT)                                           | Read When (WHEN)                                  |
| ------------ | --------------------------------------------------------- | ------------------------------------------------- |
| `auth.go`    | Authenticator, gRPC interceptor, tenant ID context        | Implementing authentication, debugging auth flow  |
| `hmac.go`    | API key parsing, HMAC-SHA256 computation, format helpers  | Generating keys, validating key format            |
| `errors.go`  | 5-tier error taxonomy (missing, invalid, revoked)         | Mapping errors to gRPC codes, debugging failures  |
| `README.md`  | Architecture decisions, invariants, auth flow             | Understanding HMAC design, troubleshooting issues |
