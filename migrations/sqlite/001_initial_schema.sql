-- Initial schema for SQLite
-- All timestamps stored as TEXT in RFC 3339 format (UTC)
-- UUIDs stored as CHAR(36) with hyphens

CREATE TABLE IF NOT EXISTS tenants (
    tenant_id CHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    deleted_at TEXT,
    CHECK (created_at LIKE '____-__-__T__:__:__Z'),
    CHECK (modified_at LIKE '____-__-__T__:__:__Z'),
    CHECK (deleted_at IS NULL OR deleted_at LIKE '____-__-__T__:__:__Z')
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
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id),
    CHECK (created_at LIKE '____-__-__T__:__:__Z'),
    CHECK (modified_at LIKE '____-__-__T__:__:__Z'),
    CHECK (deleted_at IS NULL OR deleted_at LIKE '____-__-__T__:__:__Z')
);

CREATE TABLE IF NOT EXISTS events (
    event_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    client_timestamp TEXT NOT NULL,
    server_received_at TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_offset INTEGER NOT NULL,
    payload_hash TEXT NOT NULL,
    matched_rule_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id),
    CHECK (client_timestamp LIKE '____-__-__T__:__:__Z'),
    CHECK (server_received_at LIKE '____-__-__T__:__:__Z'),
    CHECK (created_at LIKE '____-__-__T__:__:__Z')
);

CREATE TABLE IF NOT EXISTS users (
    user_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL,
    force_password_change INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id),
    CHECK (created_at LIKE '____-__-__T__:__:__Z'),
    CHECK (modified_at LIKE '____-__-__T__:__:__Z'),
    CHECK (deleted_at IS NULL OR deleted_at LIKE '____-__-__T__:__:__Z')
);

CREATE TABLE IF NOT EXISTS sessions (
    token TEXT PRIMARY KEY,
    data BLOB NOT NULL,
    expiry REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS event_rule_matches (
    event_id CHAR(36) NOT NULL,
    rule_id CHAR(36) NOT NULL,
    matched_at TEXT NOT NULL,
    PRIMARY KEY (event_id, rule_id),
    FOREIGN KEY (event_id) REFERENCES events(event_id),
    FOREIGN KEY (rule_id) REFERENCES rules(rule_id),
    CHECK (matched_at LIKE '____-__-__T__:__:__Z')
);

CREATE TABLE IF NOT EXISTS migrations (
    migration_id TEXT PRIMARY KEY,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL,
    execution_ms INTEGER NOT NULL,
    CHECK (applied_at LIKE '____-__-__T__:__:__Z')
);
