# ADR-003: UUID Strategy (UUIDv7)

Date: 2025-10-28

## Context

TrapperKeeper requires globally unique identifiers for all entities (tenants, users, rules, events, API keys, sessions, etc.). We need identifiers that are:
- Globally unique across distributed systems
- Sortable for time-series queries and debugging
- Efficient for database indexing
- Compatible with multiple database backends (SQLite, PostgreSQL, MySQL)

Traditional approaches:
- **Auto-increment integers**: Not globally unique, require coordination
- **UUIDv4 (random)**: Not sortable, poor database index performance
- **Snowflake IDs**: Custom implementation, not standard

## Decision

Use UUIDv7 for all system identifiers.

UUIDv7 provides:
- **Time-ordered sortability**: Timestamp prefix enables natural chronological ordering
- **Global uniqueness**: No coordination required between instances
- **Standard format**: RFC-compliant UUID structure
- **Time-series optimization**: Natural clustering for time-based queries

### Implementation Details

**Storage Format:**
- PostgreSQL/MySQL: Use native UUID type for speed and memory efficiency
- SQLite: Use string representation (no native UUID type)
- All UUIDs stored/transmitted in canonical 8-4-4-4-12 format

**Primary Keys:**
- UUIDs are always primary keys in relational databases
- Follow naming convention: `<type>_id` (e.g., `rule_id`, `user_id`, `tenant_id`)

**Generation:**
- Server-generated: For entities created through API (rules, users, API keys, sessions)
- Client-generated: For events (event_id) to enable offline operation and deduplication

### Entities Using UUIDv7

All system entities:
- **Tenants**: `tenant_id`
- **Teams**: `team_id`
- **Users**: `user_id`
- **Rules**: `rule_id`
- **API Keys**: `api_key_id`
- **Events**: `event_id` (client-generated)
- **Sessions**: `session_id`
- **HMAC Secrets**: `hmac_secret_id`
- **Internal Relations**: `or_group_id`, `condition_id`, `field_id`, `scope_id`

### Clock Synchronization

**Requirements:**
- Systems should maintain NTP synchronization
- Accept client-generated UUIDv7s as-is (no server-side validation)
- Warn if client/server time differs by >100ms during clock drift detection

**Event Handling:**
- Include both `client_timestamp` and `server_received_at` for correlation
- Use server time for critical operations (event bucketing, file rotation)

## Consequences

**Pros:**
- Natural time-ordering simplifies debugging and time-series queries
- Global uniqueness eliminates coordination overhead
- Standard format ensures compatibility across systems
- Efficient database indexing due to sortable nature
- Client-side generation enables offline event creation
- No central ID allocation service required

**Cons:**
- Larger than auto-increment integers (128 bits vs 32/64 bits)
- Exposes approximate creation time (acceptable for our use case)
- Requires consistent time synchronization across systems
- SQLite uses string representation (less efficient than native UUID types)

**Future Considerations:**
- If storage size becomes critical, consider hybrid approach with integers for high-volume internal relations
- Monitor database performance metrics to validate indexing efficiency
- Document UUID generation library choices per language SDK

## Related Decisions

**Depends on:**
- **ADR-001: Architectural Principles** - Implements the Consistent Encoding and Identifiers principle by standardizing on UUIDv7 for all system identifiers

**Used by:**
- **ADR-019: Event Schema and Storage** - Uses UUIDv7 for event_id generation
