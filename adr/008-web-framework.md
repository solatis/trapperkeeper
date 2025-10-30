# ADR-008: Web Framework Selection

## Revision Log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper requires HTTP routing for a server-side rendered web UI service (`tk-web-ui`). We strongly prefer small, composable libraries over full frameworks, evaluated Axum, actix-web, Rocket, and Warp for composability, performance, and Tower ecosystem integration.

## Decision

We will use Axum as the HTTP web framework by integrating it with Tower middleware, askama templates for server-side rendering, and tower-sessions for database-backed session management.

## Consequences

**Benefits:**
- Composable middleware with Tower ecosystem enables clear middleware ordering and composition
- Strong type safety prevents common web vulnerabilities
- Excellent SSR support with askama templates aligns with no-JavaScript requirement
- Integrates seamlessly with sqlx for database-backed sessions
- High performance with efficient async runtime and small dependency footprint

**Tradeoffs:**
- Requires async runtime (tokio) adding complexity for developers new to Rust async
- Tower middleware requires understanding of Service/Layer traits
- Need to assemble features ourselves (not a batteries-included framework)
- Less mature ecosystem than some alternatives like actix-web

**Operational Implications:**
- Pure server-side rendering with no JavaScript means all forms use traditional POST submissions
- Timestamp display always shows UTC in MVP (no timezone conversion)
- Session management uses database-backed storage with sliding expiration (24 hours default, 30 days with "remember me")
- Health check endpoints required for container orchestration (see ADR-009)

## Implementation

1. Integrate Axum with Tower middleware ecosystem for composable request handling
2. Configure askama template engine for server-side rendering with no JavaScript
3. Implement tower-sessions with sqlx backend for database-backed session storage
4. Build custom redirect middleware for force password change flow (see Appendix A)
5. Configure middleware chain: tower-sessions → custom redirect middleware → routes
6. Set up session expiration policies (24 hours default, 30 days with "remember me")
7. Integrate with health check endpoints as specified in ADR-009

## Related Decisions

**Depends on:**
- **ADR-006: Service Architecture** - Implements the HTTP service layer for the tk-web-ui component

**Extended by:**
- **ADR-011: Authentication and Users** - Uses the web framework for session management

## Appendix A: Session Management Implementation Details

**Architecture:**
- State: `tower-sessions` with `sqlx` backend for database-backed sessions
- Middleware chain: `tower-sessions` → custom redirect middleware → routes
- Simplicity: Minimal code for single authentication state
- Testability: Integration tests with memory store, clear migration path

**Session Storage:**
- Database-backed session storage using sqlx (sessions table)
- Sliding expiration (24 hours default, 30 days with "remember me")

**Force Password Change Flow:**
- Middleware-based redirect flow
- Login succeeds but custom middleware redirects all routes to `/change-password`
- Only `/change-password` endpoint accessible until password changed
- Clear `force_password_change` flag on password change with `session.regenerate()`
- No password requirements in MVP
- Session middleware (tower-sessions layer) for protecting authenticated routes
