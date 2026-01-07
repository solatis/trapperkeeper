-- name: get-user
SELECT user_id, tenant_id, username, password_hash, role, force_password_change, created_at, modified_at, deleted_at
FROM users
WHERE user_id = ? AND deleted_at IS NULL;

-- name: get-user-by-username
SELECT user_id, tenant_id, username, password_hash, role, force_password_change, created_at, modified_at, deleted_at
FROM users
WHERE username = ? AND deleted_at IS NULL;

-- name: list-users-by-tenant
SELECT user_id, tenant_id, username, password_hash, role, force_password_change, created_at, modified_at, deleted_at
FROM users
WHERE tenant_id = ? AND deleted_at IS NULL
ORDER BY created_at DESC;

-- name: insert-user
INSERT INTO users (user_id, tenant_id, username, password_hash, role, force_password_change, created_at, modified_at, deleted_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL);

-- name: update-user-password
UPDATE users
SET password_hash = ?, force_password_change = ?, modified_at = ?
WHERE user_id = ? AND deleted_at IS NULL;

-- name: delete-user
UPDATE users
SET deleted_at = ?
WHERE user_id = ?;
