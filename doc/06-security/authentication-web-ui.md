---
doc_type: spoke
status: active
primary_category: security
hub_document: doc/06-security/README.md
tags:
  - authentication
  - web-ui
  - cookies
  - bcrypt
  - csrf
---

# Web UI Authentication

## Context

The Web UI requires authentication to control access to rule management, user administration, and system configuration. This document specifies cookie-based session authentication for human users accessing TrapperKeeper through web browsers.

**Hub Document**: This document is part of the [Security Architecture](README.md). See [Security Hub](README.md) Section 2 for strategic overview of dual authentication strategy and why cookie-based authentication is appropriate for interactive human users.

## Cookie-Based Session Authentication

TrapperKeeper uses **scs (alexedwards/scs)** for session management with a custom database store adapter.

### Implementation Components

**Session Storage**:

- Custom `Store` interface implementation wraps TrapperKeeper's database layer
- Preserves database backend flexibility (SQLite/PostgreSQL/MySQL) without depending on SCS-specific stores
- Session data encrypted by scs before persistence
- Automated expiry management via background cleanup task
- Session cleanup runs once per hour

**Cross-Reference**: See [Database Backend](../09-operations/database-backend.md) for sessions table schema.

**Password Hashing**:

- Bcrypt with cost factor 12 (balances security and performance)
- Unique salt embedded in each hash (no separate salt storage)
- Hash format: `$2b$12$<salt><hash>` (60 characters)
- Only hash stored in database, never plaintext passwords

**User Roles**:

TrapperKeeper implements role-based access control with three roles in a strict hierarchy:

| Role       | Permissions                                                                    |
| ---------- | ------------------------------------------------------------------------------ |
| `admin`    | User management (create/delete users, assign roles) + all operator permissions |
| `operator` | Create/edit/delete rules, view events, system configuration                    |
| `observer` | Read-only access: view rules and events, no mutations                          |

Role enforcement:

- Roles apply to Web UI only; Sensor API uses separate HMAC key authentication
- Users have exactly one role (no multi-role assignment)
- Default role for new users: `observer` (explicit selection required in UI, least privilege)
- Role stored in users table, checked on each request via authorization middleware

### Session Lifecycle

**Login Flow**:

1. User submits username/password via `/login` form
2. Server queries users table for username
3. Bcrypt verifies password against stored hash (constant-time comparison)
4. On success: Create session, set httpOnly secure cookie, redirect to dashboard
5. On failure: Return error message, log authentication failure with client IP

**Session Creation**:

- Generate cryptographically secure random session ID
- Store session data in database (encrypted by scs)
- Set cookie with security attributes (see Cookie Security Configuration)
- Session lifetime:
  - Default: 24 hours of inactivity
  - Remember-me option: 30 days (when user opts in via login form checkbox)

**Session Validation** (on each request):

- Extract session cookie from request headers
- Lookup session in database by session ID
- Verify session not expired (scs handles expiry)
- Load user_id and role from session data
- Attach user context (including role) to request for authorization

**Logout Flow**:

- Delete session from database
- Clear session cookie (set expiry to past date)
- Redirect to login page

### Cookie Security Configuration

**Required Cookie Attributes**:

```go
// Configure session manager
sessionManager := scs.New()
sessionManager.Cookie.HttpOnly = true          // No JavaScript access (XSS protection)
sessionManager.Cookie.Secure = isHTTPS         // Only send over HTTPS when applicable
sessionManager.Cookie.SameSite = http.SameSiteLaxMode // CSRF protection + usability
sessionManager.Cookie.Path = "/"               // Explicit path scope
// Domain not set - defaults to exact host
```

**Secure Flag Behavior**:

- Automatically detected based on request protocol
- HTTPS requests: `secure=true`
- HTTP requests: `secure=false`
- Detection logic inspects request headers (`X-Forwarded-Proto`, `Forwarded`) for reverse proxy deployments

**SameSite=Lax**:

- Cookies sent on top-level navigation (clicking links from external sites)
- Cookies blocked on cross-site POST requests (CSRF protection)
- More permissive than Strict (avoids broken external links)
- Complements token-based CSRF protection

## Authorization

### Role-Based Access Control Middleware

Authorization middleware enforces role permissions on every request after authentication.

**Middleware Chain Order**:

1. Session validation (authentication)
2. Force password change check
3. Role authorization check
4. CSRF validation (for mutations)
5. Request handler

**Implementation**:

```go
// RequireRole returns middleware that enforces minimum role level
func RequireRole(minRole string) func(http.Handler) http.Handler {
    roleLevel := map[string]int{
        "observer": 1,
        "operator": 2,
        "admin":    3,
    }

    return func(next http.Handler) http.Handler {
        return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
            ctx := r.Context()
            userRole := sessionManager.GetString(ctx, "role")

            if roleLevel[userRole] < roleLevel[minRole] {
                http.Error(w, "Forbidden", http.StatusForbidden)
                return
            }

            next.ServeHTTP(w, r)
        })
    }
}
```

**Route Protection**:

```go
// Observer routes (read-only)
mux.Handle("GET /rules", RequireRole("observer")(listRulesHandler))
mux.Handle("GET /events", RequireRole("observer")(listEventsHandler))

// Operator routes (mutations)
mux.Handle("POST /rules", RequireRole("operator")(createRuleHandler))
mux.Handle("PUT /rules/{id}", RequireRole("operator")(updateRuleHandler))
mux.Handle("DELETE /rules/{id}", RequireRole("operator")(deleteRuleHandler))

// Admin routes (user management)
mux.Handle("GET /users", RequireRole("admin")(listUsersHandler))
mux.Handle("POST /users", RequireRole("admin")(createUserHandler))
mux.Handle("DELETE /users/{id}", RequireRole("admin")(deleteUserHandler))
mux.Handle("PUT /users/{id}/role", RequireRole("admin")(updateUserRoleHandler))
```

**Authorization Failures**:

- Return 403 Forbidden (not 401 -- user is authenticated but lacks permission)
- Log authorization failure with user_id, requested resource, and required role
- Do not reveal what role is required in error response

## CSRF Protection

### CSRF Protection Implementation

TrapperKeeper uses **double-submit cookie pattern** for all state-changing operations (POST, PUT, DELETE).

**Token Lifecycle**:

1. Generate CSRF token on session creation
2. Store token in session data
3. Set CSRF token as httpOnly cookie (separate from session cookie)
4. Include token in hidden form field via template variable
5. Validate token on POST/PUT/DELETE requests before processing

**html/template Integration**:

```html
<form method="POST" action="/rule/create">
  <input type="hidden" name="csrf_token" value="{{ .CSRFToken }}" />
  <label for="rule_name">Rule Name:</label>
  <input type="text" id="rule_name" name="rule_name" required />
  <button type="submit">Create Rule</button>
</form>
```

**Validation**:

- Compare hidden field token against cookie token
- Both must match and be associated with valid session
- CSRF failure returns 403 Forbidden with clear error message
- Log CSRF failures for security monitoring

**Exemptions**:

- GET, HEAD, OPTIONS requests (read-only operations)
- Health check endpoints (`/health`, `/metrics`)

### Force Password Change Flow

Default admin account auto-created on first run with temporary password. Users must change password on first login.

**Implementation**:

- Custom redirect middleware checks `force_password_change` flag in session
- Only `/change-password` endpoint accessible until password changed
- Call `sessionManager.RenewToken()` on password change to prevent session fixation
- Clear `force_password_change` flag after successful password update

**Middleware Logic**:

```go
func forcePasswordChangeMiddleware(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        ctx := r.Context()
        forceChange := sessionManager.GetBool(ctx, "force_password_change")

        if forceChange {
            path := r.URL.Path
            if path != "/change-password" && path != "/logout" {
                http.Redirect(w, r, "/change-password", http.StatusSeeOther)
                return
            }
        }

        next.ServeHTTP(w, r)
    })
}
```

### Session Cleanup Strategy

scs provides built-in session expiry management configured on service startup.

**Configuration**:

```go
import (
    "github.com/alexedwards/scs/v2"
    "time"
    "context"
)

// Configure session manager with expiry cleanup
sessionManager := scs.New()
sessionManager.Lifetime = 24 * time.Hour // Default session lifetime: 24 hours of inactivity

// Remember-me session extension (set per-session based on login form checkbox)
// if rememberMe {
//     sessionManager.SetDeadline(ctx, time.Now().Add(30*24*time.Hour))
// }

// Spawn background cleanup task (using database store with cleanup support)
go func() {
    ticker := time.NewTicker(1 * time.Hour)
    defer ticker.Stop()
    for range ticker.C {
        // Database store must support DeleteExpired method
        if store, ok := sessionManager.Store.(interface{ DeleteExpired() error }); ok {
            store.DeleteExpired()
        }
    }
}()
```

**Operational Considerations**:

- Cleanup frequency: Once per hour sufficient for typical session volumes
- Database impact: Minimal (simple DELETE with indexed expires_at column)
- No manual intervention required
- Cleanup continues until service shutdown

### Auto-Provisioning Default Admin

On first run (empty users table), TrapperKeeper auto-creates default admin account.

**Implementation**:

1. Check if users table is empty (no existing users)
2. Generate default admin credentials:
   - Username: `admin`
   - Password: Random 16-character alphanumeric string
   - Role: `admin`
   - `force_password_change`: `true`
3. Hash password with bcrypt cost factor 12
4. Insert into users table with UUIDv7 identifier
5. Log admin credentials to console (WARN level) for initial login
6. Credentials unrecoverable after first display (must use password reset if lost)

**Security Considerations**:

- Default password logged once at service startup
- User forced to change password on first login
- Session regenerated after password change (prevents fixation)
- No hardcoded default credentials (random generation)

## Edge Cases and Limitations

**Known Limitations**:

- Session cleanup background task: If service crashes, orphaned sessions remain until next cleanup (mitigated by 24-hour expiry)
- Cookie secure flag detection: Relies on correct `X-Forwarded-Proto` header configuration in reverse proxy (operator responsibility)
- No multi-factor authentication (MFA) in MVP
- No password reset flow in MVP (admin must manually update database)
- No account lockout after failed login attempts in MVP

**Edge Cases**:

- Session regeneration on password change: Old session invalidated, user must re-login with new password
- Concurrent sessions: Same user can have multiple active sessions from different browsers (all share same credentials)
- Session expiry during active use: User redirected to login page mid-operation (potential data loss if form not submitted)

## Related Documents

**Dependencies** (read these first):

- [Web Framework](../09-operations/web-framework.md): HTTP middleware system, scs integration
- [TLS/HTTPS Strategy](tls-https-strategy.md): Cookie secure flag coordination with TLS deployment modes
- [Identifiers (UUIDv7)](../03-data/identifiers-uuidv7.md): UUIDv7 for user_id identifiers

**Related Spokes** (siblings in this hub):

- [Authentication (Sensor API)](authentication-sensor-api.md): Contrasts cookie-based (Web UI) vs HMAC-based (Sensor API) authentication
- [TLS/HTTPS Strategy](tls-https-strategy.md): Defines deployment modes affecting cookie secure flag behavior
- [Encryption Strategy](encryption.md): Bcrypt password hashing implementation (Section 2.1)

**Extended by**:

- [Validation Hub](../07-validation/README.md): Authentication input validation (password format, username validation, session expiry)
