# Trapperkeeper Documentation Guide for LLM Agents

## Purpose

Technical documentation for Trapperkeeper system architecture, implementation patterns, validation rules, performance characteristics, and operational procedures. Organized using hub-and-spoke pattern with cross-cutting indexes for security, performance, validation, observability, and error-handling.

## Hub

**`README.md`** - Read when understanding overall documentation structure, navigation patterns, or high-level project overview

## Files

**`error-handling-index.md`** - Read when understanding error handling patterns, failure modes, or error recovery strategies across system layers

**`observability-index.md`** - Read when implementing logging, metrics, tracing, or operational visibility features

**`performance-index.md`** - Read when optimizing latency, throughput, resource usage, or understanding cost models

**`security-index.md`** - Read when implementing authentication, encryption, threat mitigation, or compliance features

**`validation-index.md`** - Read when implementing input validation, sanitization, type checking, or schema enforcement

## Subdirectories

**`_meta/`** - Read when creating documentation, need templates or standards, maintaining hubs, or implementing validation tooling

**`01-principles/`** - Read when understanding testing philosophy, architectural principles, or foundational design decisions

**`02-architecture/`** - Read when understanding system architecture, service boundaries, API design, or SDK model

**`03-data/`** - Read when understanding data schemas, identifiers, event storage, or database backend

**`04-rule-engine/`** - Read when understanding rule expression language, field path resolution, type coercion, or rule evaluation

**`05-performance/`** - Read when implementing performance optimizations, understanding cost models, or sampling strategies

**`06-security/`** - Read when implementing authentication, encryption, TLS, or threat mitigation

**`07-validation/`** - Read when implementing validation across UI, API, runtime, or database layers

**`08-resilience/`** - Read when implementing error handling, failure modes, or degradation strategies

**`09-operations/`** - Read when understanding deployment, configuration, database operations, or CLI design

**`10-integration/`** - Read when understanding monorepo structure, package boundaries, or build architecture

**`scripts/`** - Read when running validation tools or understanding documentation automation
