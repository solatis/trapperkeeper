-- HMAC secrets and API keys schema for PostgreSQL
-- All timestamps stored as TIMESTAMP WITHOUT TIME ZONE (UTC enforced by application)
-- UUIDs stored as CHAR(32) without hyphens (hex format)

CREATE TABLE IF NOT EXISTS hmac_secrets (
    secret_id CHAR(32) PRIMARY KEY,
    secret_hash BYTEA NOT NULL,
    source TEXT NOT NULL,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    CHECK (source IN ('environment', 'auto-generated'))
);

CREATE INDEX idx_hmac_secrets_source ON hmac_secrets(source);

CREATE TABLE IF NOT EXISTS api_keys (
    api_key_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    name TEXT NOT NULL,
    key_hash BYTEA NOT NULL UNIQUE,
    secret_id CHAR(32) NOT NULL,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    last_used_at TIMESTAMP WITHOUT TIME ZONE,
    revoked_at TIMESTAMP WITHOUT TIME ZONE,
    FOREIGN KEY (secret_id) REFERENCES hmac_secrets(secret_id),
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id)
);

CREATE INDEX idx_api_keys_tenant ON api_keys(tenant_id);
CREATE INDEX idx_api_keys_secret ON api_keys(secret_id);
CREATE INDEX idx_api_keys_revoked ON api_keys(revoked_at) WHERE revoked_at IS NULL;
