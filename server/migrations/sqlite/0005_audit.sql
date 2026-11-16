-- The middleware writes one row for every non-GET request under /api/v1 that
-- matched a route, after the handler answered (ADR-0031). The table has no
-- deleted_at, because an audit row records a request and is no entity, and
-- ADR-0024 gives deleted_at to entity tables. Nothing in the server deletes
-- or prunes a row (ADR-0031).
CREATE TABLE audit (
    -- audit_id holds a version 7 UUID, so ORDER BY audit_id gives time order
    -- and the id serves as the paging cursor (ADR-0020).
    audit_id       TEXT PRIMARY KEY,
    -- The tenant that the request acted on. The middleware takes the tenant
    -- that the route resolved from its path when there is one, and the
    -- caller's tenant otherwise (ADR-0031). A login attempt, with a right or
    -- a wrong password, is anonymous and names a user, and its row is filed
    -- under that user's tenant. The column is NULL for an anonymous request
    -- that names no live user.
    tenant_id      TEXT,
    -- Who did it. By id alone, without the tenant, because the actor may be
    -- the superadmin of $tk acting on a customer tenant. NULL for an
    -- anonymous request.
    user_id        TEXT REFERENCES users (user_id),
    method         TEXT NOT NULL,
    -- The matched template, such as /api/v1/tenants/{tenant}/users/{name}.
    route          TEXT NOT NULL,
    -- The template's parameters as one JSON object. Never a body or a query
    -- string.
    path_params    TEXT NOT NULL,
    status         INTEGER NOT NULL,
    -- The TCP peer's address.
    peer_address   TEXT NOT NULL,
    -- X-Forwarded-For as received when present, else the peer.
    client_address TEXT NOT NULL,
    -- recorded_at holds the time at which the middleware wrote the row, in
    -- epoch microseconds in UTC, as every time column does (ADR-0019). The
    -- id orders the trail, and recorded_at is the time that the trail shows.
    recorded_at    BIGINT NOT NULL
);

-- A tenant's trail in time order. No read returns the rows whose tenant_id
-- is NULL. The middleware writes them all the same, because it records
-- every non-GET request that matched a route (ADR-0031).
CREATE INDEX audit_tenant_time ON audit (tenant_id, audit_id);
-- What one user did, in time order. The index serves the statement of
-- store::audit::page that filters on a user.
CREATE INDEX audit_tenant_user_time ON audit (tenant_id, user_id, audit_id);
