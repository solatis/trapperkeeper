-- The same version and columns as the SQLite and Postgres files, in the
-- MariaDB forms 0001_tenants.sql explains: VARCHAR columns and the
-- utf8mb4_bin collation. No live column, because a row is never deleted.
CREATE TABLE audit (
    audit_id       VARCHAR(36)  PRIMARY KEY,
    tenant_id      VARCHAR(36),
    user_id        VARCHAR(36)  REFERENCES users (user_id),
    method         VARCHAR(16)  NOT NULL,
    route          VARCHAR(255) NOT NULL,
    path_params    TEXT         NOT NULL,
    status         INTEGER      NOT NULL,
    peer_address   VARCHAR(64)  NOT NULL,
    client_address VARCHAR(255) NOT NULL,
    recorded_at    BIGINT       NOT NULL
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;

CREATE INDEX audit_tenant_time ON audit (tenant_id, audit_id);
CREATE INDEX audit_tenant_user_time ON audit (tenant_id, user_id, audit_id);
