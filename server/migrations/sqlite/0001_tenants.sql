-- tenant_id is the internal id and tenant_name is what people use. Names are
-- unique and lowercase (doc/adr/0020).
CREATE TABLE tenants (
    tenant_id   TEXT PRIMARY KEY,
    tenant_name TEXT NOT NULL UNIQUE CHECK (tenant_name = lower(tenant_name))
);

-- A fixed id, so the default tenant has the same id in every installation and
-- code can hold it as a constant. It is a version 7 UUID with an all-zero
-- timestamp, so it sorts first.
INSERT INTO tenants (tenant_id, tenant_name)
VALUES ('00000000-0000-7000-8000-000000000000', 'default');
