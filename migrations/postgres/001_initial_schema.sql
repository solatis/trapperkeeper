-- Initial schema for PostgreSQL
-- All timestamps stored as TIMESTAMP WITHOUT TIME ZONE (UTC enforced by application)
-- UUIDs stored as CHAR(36) for SQLite compatibility

CREATE TABLE IF NOT EXISTS tenants (
    tenant_id CHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    modified_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    deleted_at TIMESTAMP WITHOUT TIME ZONE
);

CREATE TABLE IF NOT EXISTS rules (
    rule_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    action TEXT NOT NULL,
    sample_rate REAL NOT NULL DEFAULT 1.0,
    scope_tags TEXT NOT NULL,
    expression TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    modified_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    deleted_at TIMESTAMP WITHOUT TIME ZONE,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id)
);

CREATE TABLE IF NOT EXISTS events (
    event_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    client_timestamp TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    server_received_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    file_path TEXT NOT NULL,
    file_offset BIGINT NOT NULL,
    payload_hash TEXT NOT NULL,
    matched_rule_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id)
);

CREATE TABLE IF NOT EXISTS users (
    user_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL,
    force_password_change BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    modified_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    deleted_at TIMESTAMP WITHOUT TIME ZONE,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY,
    data BYTEA NOT NULL,
    expiry TIMESTAMP WITHOUT TIME ZONE NOT NULL
);

CREATE TABLE IF NOT EXISTS event_rule_matches (
    event_id CHAR(36) NOT NULL,
    rule_id CHAR(36) NOT NULL,
    matched_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    PRIMARY KEY (event_id, rule_id),
    FOREIGN KEY (event_id) REFERENCES events(event_id),
    FOREIGN KEY (rule_id) REFERENCES rules(rule_id)
);

CREATE TABLE IF NOT EXISTS migrations (
    migration_id TEXT PRIMARY KEY,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
    execution_ms INTEGER NOT NULL
);
