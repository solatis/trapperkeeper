# 0013. Where secrets may come from

Date: 2026-11-01.

## Context

The milestone M1 first said that secrets come only from the environment. The
owner revised that on 2026-11-01. A file is a normal source of secrets, as
Kubernetes mounts and TLS certificates show. A signing or encryption key in
the configuration file is acceptable.

## Decision

A secret setting may be given in the configuration file or in the
environment. On the command line it is accepted with a warning, because the
process list shows it. User and administrator credentials are never
configuration, because the store holds them.

## Rejected alternatives

The environment as the only source, because it is too strict for secrets
mounted as files.

## Consequences

The secret settings are the database URL, because it may carry a password, and
the signing keys. Each secret setting has a type that masks it when it is
serialized or printed, so the logged settings never show it. A secret given as a
flag is accepted, and the server logs a warning that names the flag.
`doc/architecture.md`, Cross-cutting, Operations, states the rule.
