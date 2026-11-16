-- A role is a row per tenant, with a name and a display title, so a tenant
-- may hold roles of its own (ADR-0023).
CREATE TABLE roles (
    role_id    TEXT PRIMARY KEY,
    tenant_id  TEXT NOT NULL REFERENCES tenants (tenant_id),
    role_name  TEXT NOT NULL CHECK (role_name = lower(role_name)),
    role_title TEXT NOT NULL,
    deleted_at BIGINT,
    -- The composite foreign key from users (tenant_id, role_id) needs a unique
    -- key on these two columns to reference, although role_id alone is the
    -- primary key.
    UNIQUE (tenant_id, role_id)
);

CREATE UNIQUE INDEX roles_live_name ON roles (tenant_id, role_name) WHERE deleted_at IS NULL;

-- The system tenant holds one role, superadmin (ADR-0028). The role has a
-- fixed id, a version 7 UUID with an all-zero timestamp like the system
-- tenant's id, so that code and tests can name it (ADR-0023).
INSERT INTO roles (role_id, tenant_id, role_name, role_title) VALUES
    ('00000000-0000-7000-8000-000000000001', '00000000-0000-7000-8000-000000000000', 'superadmin', 'Superadmin');
