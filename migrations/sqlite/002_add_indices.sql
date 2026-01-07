-- Performance indices for SQLite
-- Composite indices on (tenant_id, deleted_at) for multi-tenancy filtering

CREATE INDEX IF NOT EXISTS idx_tenants_deleted ON tenants(deleted_at);

CREATE INDEX IF NOT EXISTS idx_rules_tenant_deleted ON rules(tenant_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_rules_state ON rules(state) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_events_tenant_time ON events(tenant_id, server_received_at);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);

CREATE INDEX IF NOT EXISTS idx_users_tenant_deleted ON users(tenant_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_sessions_expiry ON sessions(expiry);

CREATE INDEX IF NOT EXISTS idx_matches_event ON event_rule_matches(event_id);
CREATE INDEX IF NOT EXISTS idx_matches_rule ON event_rule_matches(rule_id);
CREATE INDEX IF NOT EXISTS idx_matches_time ON event_rule_matches(matched_at);
