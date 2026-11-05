-- The same version and columns as the SQLite and Postgres files, in the
-- MariaDB forms 0001_tenants.sql explains: VARCHAR columns, the utf8mb4_bin
-- collation, and the live column in the unique key.
CREATE TABLE users (
    user_id              VARCHAR(36)  PRIMARY KEY,
    tenant_id            VARCHAR(36)  NOT NULL REFERENCES tenants (tenant_id),
    user_name            VARCHAR(255) NOT NULL CHECK (user_name = lower(user_name)),
    role_id              VARCHAR(36)  NOT NULL,
    password_hash        VARCHAR(255) NOT NULL,
    password_must_change BOOLEAN      NOT NULL,
    tokens_valid_after   BIGINT       NOT NULL,
    created_at           BIGINT       NOT NULL,
    deleted_at           BIGINT,
    live                 TINYINT AS (IF(deleted_at IS NULL, 1, NULL)) VIRTUAL,
    UNIQUE KEY users_live_name (tenant_id, user_name, live),
    FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, role_id)
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;
