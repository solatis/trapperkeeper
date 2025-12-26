# Architecture Guide for LLM Agents

## Purpose

Two-service architecture (gRPC sensor API + HTTP web UI) with unified binary distribution, SDK model, and Go module structure.

## Hub

**`README.md`** - Read when understanding two-service model, service boundaries, or overall system architecture

## Files

**`service-architecture.md`** - Read when understanding service separation rationale, gRPC vs HTTP boundaries, or deployment model

**`api-service.md`** - Read when implementing gRPC sensor API, ETAG synchronization, or HMAC authentication with grpc-go

**`sdk-model.md`** - Read when understanding SDK architecture, client-side rule evaluation, or gRPC client implementation patterns

**`binary-distribution.md`** - Read when implementing Go subcommands (cobra), understanding unified binary strategy, or build architecture with go build
