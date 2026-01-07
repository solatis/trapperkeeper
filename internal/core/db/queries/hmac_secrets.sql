-- name: get-hmac-secret
SELECT secret_hash
FROM hmac_secrets
WHERE secret_id = ?;

-- name: upsert-hmac-secret
INSERT INTO hmac_secrets (secret_id, secret_hash, source, created_at)
VALUES (?, ?, ?, ?)
ON CONFLICT (secret_id) DO UPDATE SET
    secret_hash = EXCLUDED.secret_hash,
    source = EXCLUDED.source;
