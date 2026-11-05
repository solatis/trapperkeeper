-- The same version and columns as the SQLite and MariaDB files; the three
-- directories change together.
CREATE TABLE roles (
    role_id    TEXT PRIMARY KEY,
    tenant_id  TEXT NOT NULL REFERENCES tenants (tenant_id),
    role_name  TEXT NOT NULL CHECK (role_name = lower(role_name)),
    role_title TEXT NOT NULL,
    deleted_at BIGINT,
    UNIQUE (tenant_id, role_id)
);

CREATE UNIQUE INDEX roles_live_name ON roles (tenant_id, role_name) WHERE deleted_at IS NULL;

INSERT INTO roles (role_id, tenant_id, role_name, role_title) VALUES
    ('00000000-0000-7000-8000-000000000001', '00000000-0000-7000-8000-000000000000', 'admin', 'Admin'),
    ('00000000-0000-7000-8000-000000000002', '00000000-0000-7000-8000-000000000000', 'operator', 'Operator'),
    ('00000000-0000-7000-8000-000000000003', '00000000-0000-7000-8000-000000000000', 'observer', 'Observer');
