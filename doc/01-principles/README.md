---
doc_type: hub
status: active
primary_category: architecture
title: Architectural Principles
nav_order: 1
has_children: true
consolidated_spokes:
  - schema-agnostic-architecture.md
  - least-intrusive-defaults.md
  - ephemeral-sensors.md
  - simplicity.md
  - consistent-encoding-identifiers.md
  - testing-philosophy.md
  - testing-integration-patterns.md
  - testing-examples.md
tags:
  - principles
  - schema-agnostic
  - testing
  - simplicity
---

# Architectural Principles

TrapperKeeper's architecture prioritizes **pragmatic minimalism** over enterprise complexity—building working systems before optimizing, failing gracefully rather than strictly, and aligning with modern ephemeral infrastructure.

## Core Principles

### 1. Schema-Agnostic Architecture

**The central server has zero understanding of data schemas.** Rules operate on abstract field paths resolved at runtime by SDKs. No schema registry, no pre-registration required.

Benefits: Deployment simplicity, server statelessness, schema evolution without coordination.

Tradeoffs: Limited server-side validation, runtime errors for type mismatches, field name brittleness.

See: [Schema-Agnostic Architecture](schema-agnostic-architecture.md)

### 2. Least Intrusive by Default

**System degrades to pass-through rather than failing pipelines.** Network failures trigger fail-safe mode (disable rules), missing fields skip rules, type coercion failures continue evaluation.

Benefits: Pipeline safety, operational simplicity, schema tolerance, gradual rollout.

Tradeoffs: Silent failures may hide misconfigurations, event loss on network issues, stale rules in fail-closed mode.

See: [Least Intrusive by Default](least-intrusive-defaults.md)

### 3. Ephemeral Sensors

**Sensors are short-lived by design, tied to job lifecycles.** No persistent identity, no registration, in-memory state only. Sensors live for minutes to hours and disappear when jobs complete.

Benefits: Operational simplicity, alignment with modern infrastructure, scalability, simplified SDK.

Tradeoffs: No cross-job continuity, event loss on crash, no historical sensor view, cache inefficiency.

See: [Ephemeral Sensors](ephemeral-sensors.md)

### 4. Simplicity

**Avoid over-engineering, defer complexity to future iterations.** Single-tenant only, no staged rollouts, simple database validation, JSONL storage, no automatic retention, last-write-wins concurrency. YAGNI principle applied aggressively.

Benefits: Development velocity, reduced maintenance, clear migration path, easier debugging.

Tradeoffs: Feature gaps, storage inefficiency, migration overhead, operational burden, concurrency risks.

See: [Simplicity (Pragmatic Minimalism)](simplicity.md)

### 5. Consistent Encoding and Identifiers

**Use UTF-8 everywhere and UUIDv7 for all identifiers.** All text stored as UTF-8, all entities use time-ordered UUIDv7 for globally unique, sortable identifiers.

Benefits: Encoding consistency, global uniqueness, time-series efficiency, index performance.

Tradeoffs: Clock dependency (NTP required), storage overhead, reduced human readability, timestamp precision limits.

See: [Consistent Encoding and Identifiers](consistent-encoding-identifiers.md)

### 6. Integration-First Testing

**Test at the highest meaningful level that validates business value.**

Testing priorities:

- **Integration over isolation**: Full SDK → gRPC → Database flows
- **Property-based over example-based**: Generate test data exploring schema variations
- **Ephemeral over persistent**: Isolated Docker containers with fresh state
- **Business value over coverage**: Focus on authentication, rule evaluation, events
- **Selective unit testing**: Reserve for complex logic, type coercion, performance

**Why:** Five-engineer team cannot maintain extensive test pyramids. Integration tests provide maximum confidence with minimal maintenance.

**Cross-references:**

- [Testing Philosophy](testing-philosophy.md) - Core principles and rationale
- [Testing Integration Patterns](testing-integration-patterns.md) - Docker setup, patterns
- [Testing Examples](testing-examples.md) - Concrete test implementations

### 7. Determinism for Testability

**Determinism is important—we leave no ambiguities as determinism makes systems easier to test.**

Implications:

- Prefer deterministic algorithms when feasible
- Define explicit tie-breaking mechanisms when ordering required
- Document any intentional non-determinism with rationale
- Accept theoretical non-determinism at nanosecond precision (practical tradeoff)

**Why:** Deterministic behavior enables reproducible tests, easier debugging, and predictable system behavior.

**Cross-references:**

- [Identifiers (UUIDv7)](../03-data/identifiers-uuidv7.md) - Time-ordered deterministic IDs
- [Type System and Coercion](../04-rule-engine/type-system-coercion.md) - Deterministic coercion rules

## Data Validation Vision

**Validate early, validate often in development, validate once in production.**

Validation layers:

1. **UI Prevention**: Make invalid states impossible to create (disable options, validate inputs)
2. **API Validation**: Validate before writing to database (rule creation, updates)
3. **Database Constraints**: Foreign keys and unique indexes encouraged
4. **Runtime Validation**: Validate when reading from database, optimize for release vs debug
5. **Debug vs Release Mode**:
   - Release: Validate once when retrieved
   - Debug: Validate frequently for development confidence

**Centralization**: Validation logic centralized in `tk-types` library, reused across SDK, web-ui, and api-server.

**Cross-references:**

- [Unified Validation Strategy](../07-validation/README.md) - Complete validation architecture

## Benefits

1. **Deployment Simplicity**: No schema pre-registration, server stays stateless
2. **Pipeline Safety**: Fail-safe defaults prevent observability issues from breaking flows
3. **Operational Flexibility**: Ephemeral design aligns with container/serverless patterns
4. **Development Velocity**: MVP simplicity enables fast iteration
5. **Data Consistency**: UTF-8 and UUIDv7 standards eliminate bugs
6. **Schema Evolution**: Schema-agnostic handles changes without coordination
7. **Testing Confidence**: Integration-first validates business flows
8. **Reproducible Behavior**: Deterministic design enables reliable testing
9. **Early Error Detection**: Multi-layer validation catches issues before production
10. **Performance Optimization**: Debug vs release modes balance confidence and speed

## Tradeoffs

1. **Limited Validation**: Cannot validate field existence on server side
2. **Error Visibility**: Silent failures in fail-safe mode may hide issues
3. **Feature Gaps**: MVP simplicity defers enterprise features
4. **Storage Inefficiency**: No normalization in initial implementation
5. **Clock Dependency**: UUIDv7 requires NTP synchronization
6. **Migration Overhead**: Simple MVP implementations require later refactoring
7. **Test Speed**: Integration tests slower than unit tests
8. **Determinism Constraints**: Some optimizations avoided for determinism
9. **Debug Mode Overhead**: Development builds carry validation overhead
10. **Validation Duplication**: Multi-layer validation requires consistency

## Operational Implications

1. **NTP Dependency**: UUIDv7 requires synchronized clocks
2. **No State Management**: Ephemeral sensors mean no persistent inventory
3. **Manual Retention**: Events accumulate until manual cleanup
4. **Silent Degradation**: Fail-safe can hide misconfigurations
5. **Build Modes**: Use release builds in production
6. **Validation Strategy**: Centralized in tk-types requires coordinated updates

## Implementation Notes

These principles guide implementation decisions across all components:

- **SDK Design**: Ephemeral sensors with fail-safe defaults, no schema validation
- **Rule Engine**: Field paths resolved at runtime, `on_missing_field="skip"` default
- **Event Storage**: JSONL files for MVP, UUIDv7 event IDs, full snapshots
- **API Architecture**: Stateless gRPC protocol, no sensor registration
- **Failure Handling**: Fail-safe default, network partitions disable rules
- **Data Validation**: UTF-8 everywhere, multi-layer validation, centralized in tk-types
- **Determinism**: UUIDv7 time-ordered IDs, explicit tie-breaking, deterministic coercion
- **Identifier Management**: UUIDv7 for all entities, clock drift warnings

## Related Documentation

- [SDK Model](../02-architecture/sdk-model.md) - Implements Ephemeral Sensors
- [Identifiers (UUIDv7)](../03-data/identifiers-uuidv7.md) - Implements Consistent Identifiers
- [Database Backend](../09-operations/database-backend.md) - Implements Simplicity
- [API Service Architecture](../02-architecture/api-service.md) - Implements Schema-Agnostic
- [Rule Expression Language](../04-rule-engine/expression-language.md) - Implements Schema-Agnostic
- [Failure Modes](../08-resilience/failure-modes.md) - Implements Least Intrusive
- [Testing Philosophy](testing-philosophy.md) - Implements Integration-First Testing
- [Unified Validation](../07-validation/README.md) - Implements Data Validation Vision
- [Error Handling](../08-resilience/README.md) - Comprehensive error strategy

## Future Considerations

These principles may evolve as the system matures:

1. **Strict Mode Environments** (optional):
   - Fail-closed by default for critical validation
   - Require schema registration
   - Type checking at rule definition time

2. **Multi-Tenant Enforcement**:
   - Tenant isolation and quotas
   - Per-tenant feature flags
   - Needed when cloud service deployed

These principles prioritize **simplicity and safety** over **features and strictness**. Future enhancements should justify complexity against operational burden.
