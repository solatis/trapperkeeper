# Operations Guide for LLM Agents

## Purpose

Operational architecture including multi-source configuration (viper), database backend support (SQLite/PostgreSQL/MySQL), explicit migrations (golang-migrate), cobra CLI, and net/http web framework.

## Hub

**`README.md`** - Read when understanding operational strategy, configuration management, or deployment architecture

## Files

**`configuration.md`** - Read when implementing configuration loading with viper, understanding precedence (CLI > env > file > defaults), or TOML/environment variable patterns

**`database-backend.md`** - Read when implementing database abstraction (dotsql + jmoiron/sqlx), understanding SQLite/PostgreSQL/MySQL support, or connection pool management

**`database-migrations.md`** - Read when implementing schema migrations with golang-migrate, understanding embed.FS migration loading, or explicit migration approval patterns

**`cli-design.md`** - Read when implementing CLI with cobra, understanding subcommand structure (tk-sensor-api/tk-web-ui), or flag parsing patterns

**`web-framework.md`** - Read when implementing HTTP handlers with stdlib net/http, understanding middleware chains, or Go 1.22+ path parameter routing

**`health-endpoints.md`** - Read when implementing health checks, understanding readiness vs liveness probes, or standardized health response formats
