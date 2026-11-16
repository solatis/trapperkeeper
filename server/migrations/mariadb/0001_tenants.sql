-- This file creates the same table and seed row as sqlite/0001_tenants.sql,
-- whose comments explain them, in the MariaDB forms below. The columns are
-- VARCHAR, because MariaDB cannot index TEXT without a prefix length
-- (ADR-0019). An id is a UUID stored as 36-character text on every backend
-- (ADR-0020), hence VARCHAR(36). The other lengths are a choice that no
-- measurement backs: VARCHAR(255) for names, titles, password_hash, route
-- and client_address, VARCHAR(64) for signing_salt and peer_address, and
-- VARCHAR(16) for method. These lengths are limits, and the system has
-- limits on purpose.
-- utf8mb4_bin makes MariaDB compare text by bytes, as Postgres and SQLite do
-- for equality. Under a case-insensitive collation the lowercase CHECK on a
-- name would accept "Acme", and ADR-0020 rejects case-insensitive
-- collations.
CREATE TABLE tenants (
    tenant_id    VARCHAR(36)  PRIMARY KEY,
    tenant_name  VARCHAR(255) NOT NULL CHECK (tenant_name = lower(tenant_name)),
    tenant_title VARCHAR(255) NOT NULL,
    deleted_at   BIGINT,
    -- MariaDB has no partial index. The live column is 1 for a live row and
    -- NULL for a deleted one. A unique key treats NULLs as distinct. So the
    -- unique key over (tenant_name, live) makes only live rows compete for a
    -- name (ADR-0024).
    live         TINYINT AS (IF(deleted_at IS NULL, 1, NULL)) VIRTUAL,
    UNIQUE KEY tenants_live_name (tenant_name, live)
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;

INSERT INTO tenants (tenant_id, tenant_name, tenant_title)
VALUES ('00000000-0000-7000-8000-000000000000', '$tk', 'TrapperKeeper');
