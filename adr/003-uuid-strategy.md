# ADR-003: UUID Strategy (UUIDv7)

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper requires globally unique identifiers that are sortable, efficient for database indexing, and compatible with multiple database backends (SQLite, PostgreSQL, MySQL).

## Decision

We will use UUIDv7 for all system identifiers by adopting time-ordered, globally unique IDs that provide natural chronological sorting and eliminate coordination overhead.

## Consequences

**Benefits:**
- Natural time-ordering simplifies debugging and time-series queries
- Global uniqueness eliminates coordination overhead
- Standard RFC-compliant format ensures compatibility across systems
- Efficient database indexing due to sortable nature
- Client-side generation enables offline event creation
- No central ID allocation service required

**Tradeoffs:**
- Larger than auto-increment integers (128 bits vs 32/64 bits)
- Exposes approximate creation time (acceptable for our use case)
- Requires consistent time synchronization across systems
- SQLite uses string representation (less efficient than native UUID types)

**Operational Implications:**
- Systems should maintain NTP synchronization for clock accuracy
- Accept client-generated UUIDv7s without server-side validation
- Warn if client/server time differs by >100ms during clock drift detection
- Include both `client_timestamp` and `server_received_at` for correlation

## Implementation

1. Configure storage format based on database backend:
   - PostgreSQL/MySQL: Use native UUID type for speed and memory efficiency
   - SQLite: Use string representation (no native UUID type)
   - All UUIDs stored/transmitted in canonical 8-4-4-4-12 format

2. Establish ID generation strategy by entity type:
   - Server-generated: For entities created through API (rules, users, API keys, sessions)
   - Client-generated: For events (event_id) to enable offline operation and deduplication

3. Apply UUIDs as primary keys across all system entities:
   - Tenants (`tenant_id`), Teams (`team_id`), Users (`user_id`)
   - Rules (`rule_id`), API Keys (`api_key_id`), Events (`event_id`)
   - Sessions (`session_id`), HMAC Secrets (`hmac_secret_id`)
   - Internal Relations (`or_group_id`, `condition_id`, `field_id`, `scope_id`)

4. Implement clock synchronization handling:
   - Use server time for critical operations (event bucketing, file rotation)
   - Monitor and log clock drift warnings for troubleshooting

## Related Decisions

**Depends on:**
- **ADR-001: Architectural Principles** - Implements the Consistent Encoding and Identifiers principle by standardizing on UUIDv7 for all system identifiers

**Required by:**
- **ADR-019: Event Schema and Storage** - Uses UUIDv7 for event_id generation

## Future Considerations

- If storage size becomes critical, consider hybrid approach with integers for high-volume internal relations
- Monitor database performance metrics to validate indexing efficiency
- Document UUID generation library choices per language SDK
- Evaluate impact of UUIDv7 on database replication and sharding strategies
