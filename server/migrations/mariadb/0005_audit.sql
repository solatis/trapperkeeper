-- This file creates the same table and indexes as sqlite/0005_audit.sql, in
-- the MariaDB forms that 0001_tenants.sql explains: VARCHAR columns and the
-- utf8mb4_bin collation. The table has no live column, because the live
-- column derives from deleted_at, which the audit table does not have, and
-- serves the uniqueness of a name, which an audit row does not have.
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
