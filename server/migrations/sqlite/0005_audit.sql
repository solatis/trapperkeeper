-- One row per changing request under /api/v1, written after the handler
-- answered. A row is a record, not an entity: nothing deletes or prunes it,
-- so there is no deleted_at.
CREATE TABLE audit (
    -- A version 7 UUID, so ORDER BY audit_id is time order and the id is the
    -- paging cursor (doc/adr/0020).
    audit_id       TEXT PRIMARY KEY,
    -- The tenant the request acted on: the one in its path when it had one,
    -- else the caller's. NULL for an anonymous request that named no user.
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
    -- Epoch microseconds in UTC, as every time column (doc/adr/0019).
    recorded_at    BIGINT NOT NULL
);

-- A tenant's trail in time order.
CREATE INDEX audit_tenant_time ON audit (tenant_id, audit_id);
-- What one user did: the user filter of the read route, in time order.
CREATE INDEX audit_tenant_user_time ON audit (tenant_id, user_id, audit_id);
