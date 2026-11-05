-- The same version and columns as the SQLite and MariaDB files; the three
-- directories change together.
CREATE TABLE tenants (
    tenant_id   TEXT PRIMARY KEY,
    tenant_name TEXT NOT NULL CHECK (tenant_name = lower(tenant_name)),
    deleted_at  BIGINT
);

CREATE UNIQUE INDEX tenants_live_name ON tenants (tenant_name) WHERE deleted_at IS NULL;

INSERT INTO tenants (tenant_id, tenant_name)
VALUES ('00000000-0000-7000-8000-000000000000', 'default');
