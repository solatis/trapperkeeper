-- A role is a row per tenant, with a name and a display title, so a tenant
-- can hold roles of its own later (doc/adr/0023).
CREATE TABLE roles (
    role_id    TEXT PRIMARY KEY,
    tenant_id  TEXT NOT NULL REFERENCES tenants (tenant_id),
    role_name  TEXT NOT NULL CHECK (role_name = lower(role_name)),
    role_title TEXT NOT NULL,
    deleted_at BIGINT,
    -- A user references its role together with its tenant, so a user cannot
    -- hold another tenant's role.
    UNIQUE (tenant_id, role_id)
);

CREATE UNIQUE INDEX roles_live_name ON roles (tenant_id, role_name) WHERE deleted_at IS NULL;

-- Fixed ids, version 7 UUIDs with an all-zero timestamp like the default
-- tenant's, so code and tests can name them.
INSERT INTO roles (role_id, tenant_id, role_name, role_title) VALUES
    ('00000000-0000-7000-8000-000000000001', '00000000-0000-7000-8000-000000000000', 'admin', 'Admin'),
    ('00000000-0000-7000-8000-000000000002', '00000000-0000-7000-8000-000000000000', 'operator', 'Operator'),
    ('00000000-0000-7000-8000-000000000003', '00000000-0000-7000-8000-000000000000', 'observer', 'Observer');
