# ADR-011: Authentication and User Management

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper needs authentication for the Web UI to control access to rule management. This ADR specifically covers Web UI authentication only; API authentication for sensors and programmatic access is covered separately in ADR-012.

The system is designed for single-tenant deployment in the MVP. Multi-tenancy is out of scope except for the data model, which includes tenant_id support for future expansion.

Requirements:
- Simple authentication for web users
- Admin control over user creation
- Persistent sessions ("remember me")
- Basic role-based access (admin/user)
- No self-registration needed
- Globally unique, sortable user identifiers
- UTF-8 support for all user-generated content

## Decision

Implement cookie-based session authentication for the Web UI:
- Username/password authentication
- Bcrypt for password hashing
- Secure, httpOnly cookies for sessions
- Sessions stored in database using tower-sessions with sqlx backend
- Two roles: admin and user
- Auto-create default admin on first run
- Users identified by user_id using UUIDv7 format (see ADR 007 for system-wide UUID strategy)
- All user-generated content (usernames, etc.) stored as UTF-8
- Force password change flow implemented via middleware
  - Custom redirect middleware checks session flag and redirects to /change-password
  - Only /change-password endpoint accessible until password changed
  - Call session.regenerate() on password change to prevent session fixation

The REST API will use API keys (separate implementation, see ADR-012).

## Consequences

**Benefits:**
- Simple and well-understood pattern
- Works naturally with server-side rendering
- No JavaScript required for auth
- Easy to implement remember-me with persistent cookies
- Sessions can be revoked by deleting from database
- Middleware-based approach provides clear separation
- tower-sessions integrates seamlessly with sqlx

**Tradeoffs:**
- Need to manage session cleanup
- Cookies don't work for API/programmatic access (intended)
- Requires CSRF protection for state-changing operations
- Session storage adds database load

**Operational Implications:**
- Default admin account auto-created on first run
- Users must change password on first login
- Session regeneration occurs on password change to prevent session fixation
- Sessions persisted in database require cleanup strategy

This approach provides secure, simple authentication appropriate for an admin UI without over-engineering.

## Implementation

1. Integrate tower-sessions with sqlx backend into Axum middleware stack
2. Create users table with user_id (UUIDv7), username, password_hash (bcrypt), role, and force_password_change flag
3. Implement /login endpoint with bcrypt password verification and secure cookie creation
4. Build custom middleware to check force_password_change flag and redirect to /change-password
5. Create /change-password endpoint that calls session.regenerate() after successful password update
6. Add session cleanup job to remove expired sessions from database
7. Implement CSRF protection for state-changing operations
8. Create auto-provisioning logic for default admin account on first run

## Related Decisions

**Depends on:**
- **ADR-008: Web Framework** - Uses the Axum framework and tower-sessions for cookie-based authentication

**Related to:**
- **ADR-012: API Authentication** - Covers API authentication for sensors (this ADR covers Web UI only)