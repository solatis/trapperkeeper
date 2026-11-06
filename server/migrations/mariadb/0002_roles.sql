-- The same version and columns as the SQLite and Postgres files, in the
-- MariaDB forms 0001_tenants.sql explains: VARCHAR columns, the utf8mb4_bin
-- collation, and the live column in the unique key.
CREATE TABLE roles (
    role_id    VARCHAR(36)  PRIMARY KEY,
    tenant_id  VARCHAR(36)  NOT NULL REFERENCES tenants (tenant_id),
    role_name  VARCHAR(255) NOT NULL CHECK (role_name = lower(role_name)),
    role_title VARCHAR(255) NOT NULL,
    deleted_at BIGINT,
    live       TINYINT AS (IF(deleted_at IS NULL, 1, NULL)) VIRTUAL,
    UNIQUE KEY roles_tenant_role (tenant_id, role_id),
    UNIQUE KEY roles_live_name (tenant_id, role_name, live)
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;

INSERT INTO roles (role_id, tenant_id, role_name, role_title) VALUES
    ('00000000-0000-7000-8000-000000000001', '00000000-0000-7000-8000-000000000000', 'superadmin', 'Superadmin');
