# ADR-011: Authentication and User Management

Date: 2025-10-28

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
- Sessions stored in database
- Two roles: admin and user
- Auto-create default admin on first run
- Users identified by user_id using UUIDv7 format (see ADR 007 for system-wide UUID strategy)
- All user-generated content (usernames, etc.) stored as UTF-8

The REST API will use API keys (separate implementation, see ADR-012).

## Consequences

**Pros:**
- Simple and well-understood pattern
- Works naturally with server-side rendering
- No JavaScript required for auth
- Easy to implement remember-me with persistent cookies
- Sessions can be revoked by deleting from database

**Cons:**
- Need to manage session cleanup
- Cookies don't work for API/programmatic access (intended)
- Requires CSRF protection for state-changing operations
- Session storage adds database load

This approach provides secure, simple authentication appropriate for an admin UI without over-engineering.

## Related Decisions

**Depends on:**
- **ADR-008: Web Framework** - Uses the Chi router and session middleware for cookie-based authentication

**Relates to:**
- **ADR-012: API Authentication** - Covers API authentication for sensors (this ADR covers Web UI only)