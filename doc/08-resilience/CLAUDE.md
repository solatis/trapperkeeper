# Resilience Architecture Guide for LLM Agents

## Purpose

Error handling strategy implementing least intrusive principle with error taxonomy, degradation patterns, slog logging standards, and monitoring thresholds for Go implementation.

## Hub

**`README.md`** - Read when understanding resilience philosophy, least intrusive principle, or error handling strategy overview

## Files

**`error-taxonomy.md`** - Read when implementing error handling, understanding error categories (validation/network/database/protocol/type/field), or (T, error) return patterns

**`failure-modes.md`** - Read when implementing degradation strategies, understanding fail-safe vs fail-fast decisions, or circuit breaker patterns

**`logging-standards.md`** - Read when implementing structured logging with slog, understanding log levels, or formatting log output for Go services

**`monitoring-strategy.md`** - Read when implementing metrics with prometheus/client_golang, understanding alert thresholds, or observability patterns
