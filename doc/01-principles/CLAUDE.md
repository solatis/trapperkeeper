# Architectural Principles Guide for LLM Agents

## Purpose

Architectural principles establishing schema-agnostic design, least intrusive defaults, pragmatic minimalism, and comprehensive testing philosophy for Go implementation.

## Hub

**`README.md`** - Read when understanding core architectural principles, schema-agnostic design, or least intrusive behavior patterns

## Files

**`schema-agnostic-architecture.md`** - Read when understanding schema-agnostic design, field path resolution, runtime type handling, or why server has no schema knowledge

**`least-intrusive-defaults.md`** - Read when implementing fail-safe modes, degradation strategies, missing field handling, or understanding default behavior priorities

**`ephemeral-sensors.md`** - Read when understanding sensor lifecycle, in-memory state management, or stateless design patterns for container/serverless environments

**`simplicity.md`** - Read when making architectural decisions, evaluating feature complexity, understanding MVP scope constraints, or applying YAGNI principle

**`consistent-encoding-identifiers.md`** - Read when implementing UTF-8 handling, UUIDv7 generation, time-ordered identifiers, or understanding encoding standards

**`testing-philosophy.md`** - Read when understanding testing strategy, test categorization (unit/integration/property), or Go testing.T patterns

**`testing-integration-patterns.md`** - Read when implementing integration tests, database fixtures, or understanding test isolation strategies with Go's testing framework

**`testing-examples.md`** - Read when writing tests for rule evaluation, field resolution, type coercion, or needing concrete test examples using testing.T
