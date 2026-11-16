-- tenant_id is the internal id and tenant_name is what people use. Names are
-- lowercase (ADR-0020).
CREATE TABLE tenants (
    tenant_id    TEXT PRIMARY KEY,
    tenant_name  TEXT NOT NULL CHECK (tenant_name = lower(tenant_name)),
    -- A display name that keeps its case (ADR-0020).
    tenant_title TEXT NOT NULL,
    -- A deleted tenant keeps its row with deleted_at set, so references to it
    -- and reports keep working (ADR-0024). Lookups of tenants filter on
    -- deleted_at IS NULL.
    deleted_at   BIGINT
);

-- Only live tenants compete for a name, so a deleted tenant's name can be
-- used again (ADR-0024).
CREATE UNIQUE INDEX tenants_live_name ON tenants (tenant_name) WHERE deleted_at IS NULL;

-- The row is the system tenant $tk, which holds the superadmin user
-- (ADR-0028). The row has a fixed id, so the id is the same in every
-- installation and the server holds it as the constant SYSTEM_TENANT_ID. The
-- id is a version 7 UUID with an all-zero timestamp. The name starts with $,
-- and only the server gives names that start with $ (ADR-0028).
INSERT INTO tenants (tenant_id, tenant_name, tenant_title)
VALUES ('00000000-0000-7000-8000-000000000000', '$tk', 'TrapperKeeper');
