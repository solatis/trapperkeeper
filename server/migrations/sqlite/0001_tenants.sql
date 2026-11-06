-- tenant_id is the internal id and tenant_name is what people use. Names are
-- unique among live tenants and lowercase (doc/adr/0020, doc/adr/0024).
CREATE TABLE tenants (
    tenant_id    TEXT PRIMARY KEY,
    tenant_name  TEXT NOT NULL CHECK (tenant_name = lower(tenant_name)),
    -- A display name that keeps its case (doc/adr/0020).
    tenant_title TEXT NOT NULL,
    -- A deleted tenant keeps its row, so references to it and reports keep
    -- working; every read filters it out (doc/adr/0024).
    deleted_at   BIGINT
);

-- Only live tenants compete for a name, so a deleted tenant's name can be
-- used again (doc/adr/0024).
CREATE UNIQUE INDEX tenants_live_name ON tenants (tenant_name) WHERE deleted_at IS NULL;

-- The system tenant, which holds the superadmin (doc/adr/0028). A fixed id,
-- so it has the same id in every installation and code can hold it as a
-- constant (doc/adr/0023). It is a version 7 UUID with an all-zero timestamp,
-- so it sorts first. Its name starts with `$`, which only the server uses for
-- names.
INSERT INTO tenants (tenant_id, tenant_name, tenant_title)
VALUES ('00000000-0000-7000-8000-000000000000', '$tk', 'TrapperKeeper');
