-- name: get-event
SELECT event_id, tenant_id, client_timestamp, server_received_at, file_path, file_offset, payload_hash, matched_rule_count, created_at
FROM events
WHERE event_id = ?;

-- name: list-events-by-time
SELECT event_id, tenant_id, client_timestamp, server_received_at, file_path, file_offset, payload_hash, matched_rule_count, created_at
FROM events
WHERE tenant_id = ? AND server_received_at BETWEEN ? AND ?
ORDER BY server_received_at DESC;

-- name: insert-event
INSERT INTO events (event_id, tenant_id, client_timestamp, server_received_at, file_path, file_offset, payload_hash, matched_rule_count, created_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);

-- name: insert-event-rule-match
INSERT INTO event_rule_matches (event_id, rule_id, matched_at)
VALUES (?, ?, ?);

-- name: list-event-rules
SELECT r.rule_id, r.name, r.action, m.matched_at
FROM event_rule_matches m
JOIN rules r ON m.rule_id = r.rule_id
WHERE m.event_id = ?
ORDER BY m.matched_at;
