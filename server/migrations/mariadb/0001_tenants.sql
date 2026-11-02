-- The same version and columns as the SQLite and Postgres files. The columns
-- are VARCHAR, because MariaDB cannot index TEXT without a prefix length.
-- utf8mb4_bin compares text by bytes, as Postgres and SQLite do, so a title
-- keeps its case in comparisons (doc/adr/0020).
CREATE TABLE tenants (
    tenant_id   VARCHAR(36)  PRIMARY KEY,
    tenant_name VARCHAR(255) NOT NULL UNIQUE CHECK (tenant_name = lower(tenant_name))
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;

INSERT INTO tenants (tenant_id, tenant_name)
VALUES ('00000000-0000-7000-8000-000000000000', 'default');
