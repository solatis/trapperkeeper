# 0013. Where secrets may come from

Date: 2026-11-02.

## Context

The milestone first said that secrets come only from the environment. The
owner revised that on 2026-11-01: a file is a normal source of secrets, as
Kubernetes mounts and TLS certificates show, and a signing or encryption key
in the configuration file is acceptable. The milestone description was
rewritten to the rule below.

## Decision

A secret setting may be given in the configuration file or in the
environment. On the command line it is accepted with a warning, because the
process list shows it. User and administrator credentials are never
configuration: they live in the store.

## Rejected alternatives

The environment as the only source, because it is too strict for secrets
mounted as files.

## Consequences

The database URL is the first secret, because it may carry a password. Its
settings type, `DatabaseUrl`, serializes and prints with the password replaced
by `***`, so the logged settings never show it. A secret given as a flag is
accepted, and the server logs a warning that names the flag.
`doc/architecture.md`, Cross-cutting, Operations, states the rule.
