# Security Architecture Guide for LLM Agents

## Purpose

Security architecture implementing defense-in-depth with dual authentication (cookie-based web UI + HMAC sensor API), TLS/HTTPS, encryption, and SOC2-aligned controls for Go implementation.

## Hub

**`README.md`** - Read when understanding security posture, threat model, or defense-in-depth strategy

## Files

**`authentication-web-ui.md`** - Read when implementing cookie-based authentication, session management with scs, or bcrypt password hashing using golang.org/x/crypto

**`authentication-sensor-api.md`** - Read when implementing HMAC-SHA256 API key authentication, understanding signature verification, or gRPC authentication interceptors

**`tls-https-strategy.md`** - Read when implementing TLS 1.3 with crypto/tls, understanding certificate management, or HTTP/gRPC transport security

**`configuration-security.md`** - Read when implementing secure configuration loading with viper, understanding secret management, or environment variable handling

**`encryption.md`** - Read when implementing data encryption, understanding at-rest vs in-transit encryption, or using golang.org/x/crypto primitives
