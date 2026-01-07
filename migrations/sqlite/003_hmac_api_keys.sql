-- HMAC secrets and API keys schema for SQLite
-- All timestamps stored as TEXT in RFC 3339 format (UTC)
-- UUIDs stored as CHAR(32) without hyphens (hex format)

CREATE TABLE IF NOT EXISTS hmac_secrets (
    secret_id CHAR(32) PRIMARY KEY,
    secret_hash BLOB NOT NULL,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK (created_at LIKE '____-__-__T__:__:__Z'),
    CHECK (source IN ('environment', 'auto-generated'))
);

CREATE INDEX idx_hmac_secrets_source ON hmac_secrets(source);

CREATE TABLE IF NOT EXISTS api_keys (
    api_key_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    name TEXT NOT NULL,
    key_hash BLOB NOT NULL UNIQUE,
    secret_id CHAR(32) NOT NULL,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT,
    FOREIGN KEY (secret_id) REFERENCES hmac_secrets(secret_id),
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id),
    CHECK (created_at LIKE '____-__-__T__:__:__Z'),
    CHECK (last_used_at IS NULL OR last_used_at LIKE '____-__-__T__:__:__Z'),
    CHECK (revoked_at IS NULL OR revoked_at LIKE '____-__-__T__:__:__Z')
);

CREATE INDEX idx_api_keys_tenant ON api_keys(tenant_id);
CREATE INDEX idx_api_keys_secret ON api_keys(secret_id);
CREATE INDEX idx_api_keys_revoked ON api_keys(revoked_at) WHERE revoked_at IS NULL;
