# 0020. Internal ids and unique names

Date: 2026-11-02.

## Context

The store gets its first table, and every later entity follows the naming that
table sets (owner, 2026-11-02). `doc/architecture.md` asks for time-ordered
unique ids for all entities.

## Decision

An entity's `<entity>_id` column, such as `tenant_id` or `user_id`, holds a
time-ordered UUID that the server generates. The id is internal. Logs and
references use it, and a person never types or reads it (owner, 2026-11-02).

An entity that people address has a `<entity>_name` column, such as
`tenant_name` or `user_name`. People log in with the name, and the UI shows it
(owner, 2026-11-02). A tenant name is unique, and a user name is unique within
its tenant. A constraint in the database enforces each name's uniqueness, so
an id never makes a duplicate name possible (owner, 2026-11-02).

The server converts names and other alphanumerical identifiers to lowercase
before it stores them (owner, 2026-11-02). A `CHECK` constraint on each name
column holds the stored value to lowercase.

A user-friendly display name is a title, in a `<entity>_title` column such as
`role_title`. A title keeps its case and is not an identifier (owner,
2026-11-02).

Ids are 36-character text on every backend, so one query set binds them the
same way (`doc/adr/0019`).

## Rejected alternatives

Auto-increment ids, because reading the generated id back differs per backend
(owner, 2026-11-02). The UUID as the login or display name, and names that are
unique only together with their id (owner, 2026-11-02). Mixed-case names
compared by bytes, under which "Acme" and "acme" are two tenants, and
case-insensitive collations, which differ per backend (owner, 2026-11-02).

## Consequences

Every table that names an entity has a unique constraint on the name in its
scope and a lowercase `CHECK` on it. A name always reads lowercase, and an
entity that needs a display form gets a `_title` column. The store component
cites this record.
