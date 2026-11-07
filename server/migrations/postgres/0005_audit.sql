-- The same version and columns as the SQLite and MariaDB files; the three
-- directories change together.
CREATE TABLE audit (
    audit_id       TEXT PRIMARY KEY,
    tenant_id      TEXT,
    user_id        TEXT REFERENCES users (user_id),
    method         TEXT NOT NULL,
    route          TEXT NOT NULL,
    path_params    TEXT NOT NULL,
    status         INTEGER NOT NULL,
    peer_address   TEXT NOT NULL,
    client_address TEXT NOT NULL,
    recorded_at    BIGINT NOT NULL
);

CREATE INDEX audit_tenant_time ON audit (tenant_id, audit_id);
CREATE INDEX audit_tenant_user_time ON audit (tenant_id, user_id, audit_id);
