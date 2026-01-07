-- name: get-api-key-by-secret-id
SELECT key_hash, tenant_id, revoked_at, api_key_id, last_used_at
FROM api_keys
WHERE secret_id = ?;

-- name: get-api-key-by-hash
SELECT tenant_id, revoked_at, api_key_id, last_used_at
FROM api_keys
WHERE key_hash = ?;

-- name: update-last-used
UPDATE api_keys
SET last_used_at = ?
WHERE api_key_id = ?;

-- name: insert-api-key
INSERT INTO api_keys (api_key_id, tenant_id, name, key_hash, secret_id, created_at, last_used_at, revoked_at)
VALUES (?, ?, ?, ?, ?, ?, NULL, NULL);

-- name: revoke-api-key
UPDATE api_keys
SET revoked_at = ?
WHERE api_key_id = ?;

-- name: list-api-keys-by-tenant
SELECT api_key_id, tenant_id, name, secret_id, created_at, last_used_at, revoked_at
FROM api_keys
WHERE tenant_id = ?
ORDER BY created_at DESC;
