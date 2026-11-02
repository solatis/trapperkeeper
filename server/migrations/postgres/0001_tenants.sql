-- The same version and columns as the SQLite and MariaDB files; the three
-- directories change together.
CREATE TABLE tenants (
    tenant_id   TEXT PRIMARY KEY,
    tenant_name TEXT NOT NULL UNIQUE CHECK (tenant_name = lower(tenant_name))
);

INSERT INTO tenants (tenant_id, tenant_name)
VALUES ('00000000-0000-7000-8000-000000000000', 'default');
