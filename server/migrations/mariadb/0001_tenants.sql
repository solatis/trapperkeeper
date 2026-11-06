-- The same version and columns as the SQLite and Postgres files. The columns
-- are VARCHAR, because MariaDB cannot index TEXT without a prefix length.
-- utf8mb4_bin compares text by bytes, as Postgres and SQLite do, so a title
-- keeps its case in comparisons (doc/adr/0020).
CREATE TABLE tenants (
    tenant_id    VARCHAR(36)  PRIMARY KEY,
    tenant_name  VARCHAR(255) NOT NULL CHECK (tenant_name = lower(tenant_name)),
    tenant_title VARCHAR(255) NOT NULL,
    deleted_at   BIGINT,
    -- MariaDB has no partial index. The unique key takes this column, which
    -- is 1 for a live row and NULL for a deleted one, and a unique key treats
    -- NULLs as distinct, so only live rows compete for a name.
    live         TINYINT AS (IF(deleted_at IS NULL, 1, NULL)) VIRTUAL,
    UNIQUE KEY tenants_live_name (tenant_name, live)
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;

INSERT INTO tenants (tenant_id, tenant_name, tenant_title)
VALUES ('00000000-0000-7000-8000-000000000000', '$tk', 'TrapperKeeper');
