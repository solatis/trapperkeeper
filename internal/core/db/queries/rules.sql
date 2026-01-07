-- name: get-rule
SELECT rule_id, tenant_id, name, description, action, sample_rate, scope_tags, expression, state, created_at, modified_at, deleted_at
FROM rules
WHERE rule_id = ? AND deleted_at IS NULL;

-- name: list-rules-by-tenant
SELECT rule_id, tenant_id, name, description, action, sample_rate, scope_tags, expression, state, created_at, modified_at, deleted_at
FROM rules
WHERE tenant_id = ? AND deleted_at IS NULL
ORDER BY created_at DESC;

-- name: list-rules-by-tags
SELECT rule_id, tenant_id, name, description, action, sample_rate, scope_tags, expression, state, created_at, modified_at, deleted_at
FROM rules
WHERE tenant_id = ? AND scope_tags LIKE ? AND deleted_at IS NULL
ORDER BY created_at DESC;

-- name: insert-rule
INSERT INTO rules (rule_id, tenant_id, name, description, action, sample_rate, scope_tags, expression, state, created_at, modified_at, deleted_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL);

-- name: update-rule
UPDATE rules
SET name = ?,
    description = ?,
    action = ?,
    sample_rate = ?,
    scope_tags = ?,
    expression = ?,
    state = ?,
    modified_at = ?
WHERE rule_id = ? AND deleted_at IS NULL;

-- name: delete-rule
UPDATE rules
SET deleted_at = ?
WHERE rule_id = ?;
