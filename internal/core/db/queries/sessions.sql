-- name: find-session
SELECT data, expiry
FROM sessions
WHERE token = ?;

-- name: save-session
INSERT INTO sessions (token, data, expiry)
VALUES (?, ?, ?)
ON CONFLICT (token) DO UPDATE
SET data = excluded.data, expiry = excluded.expiry;

-- name: delete-session
DELETE FROM sessions
WHERE token = ?;

-- name: delete-expired-sessions
DELETE FROM sessions
WHERE expiry < ?;
