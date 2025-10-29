# ADR-020: Client Metadata Namespace and Limits
Date: 2025-10-28

## Context

TrapperKeeper events include metadata from client sensors to enable correlation with external systems (Airflow DAGs, Kubernetes pods, batch jobs, etc.). This metadata appears in stored events and helps operators identify which pipelines generated specific events.

Two metadata sources must coexist:

1. **System metadata**: TrapperKeeper-generated fields tracking SDK version, ingestion time, server state
2. **User metadata**: Client-provided key-value pairs for correlation and debugging

Key challenges:

- **Namespace collision**: User-provided keys may conflict with system-generated fields
- **Spoofing risk**: Malicious clients could forge system metadata to mislead operators
- **Unbounded growth**: Without limits, sensors could consume excessive memory/bandwidth
- **Storage bloat**: Large metadata payloads increase event storage costs

## Decision

We will implement a **reserved namespace prefix for system metadata** and enforce strict limits on user-provided metadata.

### 1. System Metadata Prefix (`$tk.*`)

All TrapperKeeper system-generated metadata uses keys starting with `$tk.`:

**Examples**:
- `$tk.api_type`: SDK type (`"python"`, `"java"`, `"airflow"`)
- `$tk.api_version`: TrapperKeeper SDK version
- `$tk.client_ip`: IP address of client that generated event
- `$tk.client_timestamp`: Timestamp when event occurred on client (UTC)
- `$tk.server_received_at`: Timestamp when event ingested by server
- `$tk.server_version`: TrapperKeeper server version
- Framework-specific fields: `$tk.airflow_dag_id`, `$tk.airflow_task_id`, `$tk.k8s_pod_name`

**Client-side enforcement**:
- SDKs reject any custom metadata keys starting with `$`
- Clear error message: `"Metadata keys cannot start with '$' (reserved for system use)"`
- Validation happens during `sensor.add_metadata()` or sensor initialization

**Server-side enforcement**:
- API server strips any client-supplied keys starting with `$tk.`
- Server overwrites with correct system values
- Prevents spoofing of SDK version, ingestion time, or server state
- No error returned to client (silent overwrite for security)

**Rationale**: Prefix-based namespace separation is simple, visually distinct, and prevents collision. Server enforcement prevents malicious clients from forging system metadata. The `$` prefix convention aligns with templating languages and special identifiers (PHP, jQuery, etc.).

### 2. User Metadata Limits

User-provided metadata has strict limits to prevent resource exhaustion:

**Limits**:
- **Max key-value pairs**: 50 per sensor
- **Max key length**: 128 characters (UTF-8)
- **Max value length**: 1024 characters (UTF-8)
- **Total metadata size**: 64KB maximum (sum of all keys + values)
- **Character restrictions**: UTF-8 encoding, no control characters (except user-provided values may contain newlines/tabs)

**Validation**:
- Enforced client-side in SDK during `sensor.add_metadata()`
- Also enforced server-side as defense-in-depth
- Clear error messages when limits exceeded:
  - `"Metadata limit exceeded: 50 key-value pairs maximum"`
  - `"Metadata key too long: 'very_long_key_name...' (150 chars, max 128)"`
  - `"Metadata value too long for key 'description' (2048 chars, max 1024)"`
  - `"Total metadata size 80KB exceeds 64KB limit"`

**Rationale**: Generous limits accommodate common use cases (correlation IDs, job names, team identifiers) while preventing abuse. 64KB total allows ~50 pairs with reasonable value sizes. Character restrictions ensure clean storage and search.

### 3. Reserved Prefix Future-Proofing

**Current reserved prefixes**:
- `$tk.*` - TrapperKeeper system metadata

**Potential future expansions** (not implemented in MVP):
- `$user.*` - User identity fields if multi-tenant expanded
- `$env.*` - Environment detection metadata
- `$fw.*` - Framework-specific fields

**Rationale**: Single prefix sufficient for MVP. Future prefixes enable namespaced extensions without breaking existing clients.

## Consequences

### Benefits

1. **No Collisions**: System and user metadata cannot conflict due to namespace separation
2. **Spoofing Prevention**: Server enforcement prevents clients from forging system fields
3. **Resource Protection**: Limits prevent memory/bandwidth exhaustion from malicious or buggy clients
4. **Clear Errors**: Developers immediately know when they've hit limits or used reserved prefixes
5. **Storage Predictability**: Event storage size bounded by metadata limits
6. **Future Extensibility**: Prefix pattern enables new system metadata without breaking changes

### Tradeoffs

1. **Key Prefix Tax**: Users cannot use `$` prefix for their own namespacing patterns
2. **Hard Limits**: 50 key-value pairs may be insufficient for complex metadata scenarios (future: increase limit)
3. **Validation Overhead**: Client and server must validate limits on every metadata update
4. **Silent Overwrite**: Server strips `$tk.*` keys without error (security over explicitness)

### Operational Implications

1. **Documentation**: Must clearly document reserved `$tk.*` prefix in SDK guides
2. **Monitoring**: Track metadata size distribution to tune limits if needed
3. **SDK Updates**: All SDKs must enforce client-side validation consistently
4. **Event Storage**: Query interfaces can reliably filter by `$tk.*` system fields
5. **Migration**: No impact on existing events (prefix reservation is forward-compatible)

## Implementation

1. **SDK-side validation**:
   - Reject metadata keys starting with `$` during `add_metadata()` or sensor initialization
   - Validate limits: 50 pairs, 128 char keys, 1KB values, 64KB total
   - Clear error messages with specific violation details
   - Fail fast before network I/O

2. **Server-side enforcement**:
   - Strip any client-supplied keys starting with `$tk.`
   - Re-validate limits as defense-in-depth
   - Inject correct system metadata (SDK version, ingestion time, server state)
   - Log warning if client sent reserved keys (potential security issue)

3. **System metadata population**:
   - Collect at event ingestion time (not at sensor initialization)
   - Include `$tk.server_received_at`, `$tk.server_version`
   - Preserve client-provided `$tk.client_timestamp` but validate format
   - Framework wrappers set `$tk.api_type` (e.g., Airflow wrapper reports `"airflow"`, not `"python"`)

4. **Documentation**:
   - Document reserved `$tk.*` prefix in SDK reference docs
   - Provide examples of common user metadata patterns
   - Explain limit rationale and how to handle "metadata full" errors
   - List all standard `$tk.*` fields collected automatically

5. **Monitoring**:
   - Log metadata validation errors (client-side and server-side)
   - Track metadata size distribution (p50, p95, p99)
   - Alert if many clients hitting 64KB limit (may indicate need for increase)

## Related Decisions

**Depends on:**
- **ADR-019: Event Schema and Storage** - Extends event schema with metadata namespace rules and limits

**Related:**
- **ADR-001: SDK Model** - Documents metadata collection strategy and environment variable scanning

## Future Considerations

- **Dynamic limits**: Per-tenant metadata limits for power users
- **Compressed metadata**: Large values automatically compressed before transmission
- **Structured metadata types**: Support nested objects, arrays (currently flat key-value only)
- **Metadata inheritance**: Sensor groups share common metadata without duplication
- **Metadata TTL**: Time-based expiration for ephemeral correlation data
