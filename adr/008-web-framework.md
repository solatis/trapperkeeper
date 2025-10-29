# ADR-008: Web Framework Selection

Date: 2025-10-28

## Context

TrapperKeeper requires HTTP routing for a server-side rendered web UI service (`tk-web-ui`). We strongly prefer small, composable libraries over full frameworks, following Go's philosophy. The system will support both on-premise deployments and multi-tenant cloud offerings.

See [ADR-006: Service Architecture & Protocol Separation](006-service-architecture.md) for details on the two-service architecture.

Requirements:
- Standard library compatibility
- Good performance
- Minimal dependencies
- Easy integration with existing middleware
- Composability - small libraries that do one thing well
- Support for server-side rendering without JavaScript
- Session management with database-backed storage
- Health check endpoints for container orchestration

Options considered:
- `net/http` + `gorilla/mux` - mature router but no longer maintained (archived Dec 2022)
- `labstack/echo` - full-featured framework with built-in validation, rendering, middleware - violates our "small composable libraries" principle
- `go-chi/chi` - minimal router that works with standard `net/http`, excellent middleware chaining
- `julienschmidt/httprouter` - extremely fast but too limiting (no middleware support, no context propagation)

## Decision

Use Chi (`go-chi/chi/v5`) as the HTTP router for the web UI service.

## Implementation Requirements

### Web UI Constraints
- **NO JavaScript**: Pure server-side rendering using Go's `html/template`
- All forms use traditional POST submissions
- Progressive enhancement deferred to future versions
- Timestamp display: Always UTC in MVP (no timezone conversion)

See [ADR 005: Operational Endpoints](005-operational-endpoints.md) for health check and monitoring endpoint details.

### Session Management
The web framework must support:
- Database-backed session storage (sessions table)
- Sliding expiration (24 hours default, 30 days with "remember me")
- Force password change flow:
  - Login succeeds but all routes redirect to `/change-password`
  - Only `/change-password` endpoint accessible until password changed
  - No password requirements in MVP
- Session middleware for protecting authenticated routes

## Consequences

**Pros:**
- 100% compatible with `net/http` - any standard middleware works
- Very minimal - just routing, nothing else
- Composable middleware design
- Excellent performance
- Uses standard `context.Context`
- Small dependency footprint
- Aligns with no-JavaScript requirement for MVP

**Cons:**
- No built-in template rendering (use stdlib `html/template`)
- No automatic request binding (build or import as needed)
- Need to assemble features ourselves
- Session management must be implemented manually
- No built-in CSRF protection (must add middleware)

Chi aligns with Go's philosophy of composability over frameworks. We'll add specific libraries as needed rather than accepting a framework's choices. The no-JavaScript constraint simplifies the initial implementation while maintaining the option for progressive enhancement in future versions.

## Related Decisions

**Depends on:**
- **ADR-006: Service Architecture** - Implements the HTTP service layer for the tk-web-ui component

**Extended by:**
- **ADR-011: Authentication and Users** - Uses the web framework for session management
