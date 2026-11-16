-- A user belongs to one tenant and holds one of that tenant's roles
-- (ADR-0003, ADR-0023).
CREATE TABLE users (
    user_id              TEXT PRIMARY KEY,
    tenant_id            TEXT NOT NULL REFERENCES tenants (tenant_id),
    user_name            TEXT NOT NULL CHECK (user_name = lower(user_name)),
    role_id              TEXT NOT NULL,
    -- An Argon2id hash as a PHC string, which carries its own parameters and
    -- salt (ADR-0022).
    password_hash        TEXT NOT NULL,
    -- The server returns the flag to the client in the login response, the
    -- minted session, /me and the user list, so that a client such as the UI
    -- asks the user for a new password. The server refuses nothing because of
    -- the flag, and a password change clears it (ADR-0022).
    password_must_change BOOLEAN NOT NULL,
    -- Tokens issued before this time are refused, so setting it to now
    -- revokes every token of the user (ADR-0025).
    tokens_valid_after   BIGINT NOT NULL,
    -- Epoch microseconds in UTC, as every time column (ADR-0019).
    created_at           BIGINT NOT NULL,
    deleted_at           BIGINT,
    -- The role must belong to the user's tenant (ADR-0023).
    FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, role_id)
);

CREATE UNIQUE INDEX users_live_name ON users (tenant_id, user_name) WHERE deleted_at IS NULL;
