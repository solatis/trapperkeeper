-- name: get-tenant
SELECT tenant_id, name, created_at, modified_at, deleted_at
FROM tenants
WHERE tenant_id = ? AND deleted_at IS NULL;

-- name: list-tenants
SELECT tenant_id, name, created_at, modified_at, deleted_at
FROM tenants
WHERE deleted_at IS NULL
ORDER BY created_at DESC;

-- name: insert-tenant
INSERT INTO tenants (tenant_id, name, created_at, modified_at, deleted_at)
VALUES (?, ?, ?, ?, NULL);

-- name: update-tenant
UPDATE tenants
SET name = ?, modified_at = ?
WHERE tenant_id = ? AND deleted_at IS NULL;

-- name: delete-tenant
UPDATE tenants
SET deleted_at = ?
WHERE tenant_id = ?;
