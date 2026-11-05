-- The same version and columns as the SQLite and MariaDB files; the three
-- directories change together.
CREATE TABLE users (
    user_id              TEXT PRIMARY KEY,
    tenant_id            TEXT NOT NULL REFERENCES tenants (tenant_id),
    user_name            TEXT NOT NULL CHECK (user_name = lower(user_name)),
    role_id              TEXT NOT NULL,
    password_hash        TEXT NOT NULL,
    password_must_change BOOLEAN NOT NULL,
    tokens_valid_after   BIGINT NOT NULL,
    created_at           BIGINT NOT NULL,
    deleted_at           BIGINT,
    FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, role_id)
);

CREATE UNIQUE INDEX users_live_name ON users (tenant_id, user_name) WHERE deleted_at IS NULL;
