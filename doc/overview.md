# Overview

## The problem

Data pipelines run on ephemeral compute: scheduler tasks, cluster jobs,
containers, serverless functions. The data flowing through them changes shape
constantly. Teams want quality checks and visibility on that data, but the
existing options force a bad trade.

- Checks hardcoded in pipeline code need a redeploy for every change.
- Schema-registry style tools need upfront registration and break on drift.
- A monitoring tool that can fail the pipeline becomes a production risk, so
  it gets removed.

## The product

TrapperKeeper is a data-quality rule engine with central management and local
evaluation.

- A **sensor** is embedded in pipeline code through an SDK. It fetches the rules
  that apply to it, evaluates them against each record in-process (target: under
  1 ms per record, vectorized for dataframes), and reports matches.
- A central **server** stores rules and receives events. It never sees or
  understands the data schema.
- **Operators** manage rules through a web UI, a CLI, or an AI agent. A rule
  change reaches running sensors within about 30 seconds, without any deploy.
- The **AI agent** troubleshoots data problems on request. It queries events,
  and it creates, scopes, samples and pauses rules on its own, inside
  guardrails.

## Users

- Data engineers: embed the sensor, a handful of lines per pipeline.
- Operators / data-quality owners: author rules, watch events.
- Platform teams: deploy and run the server.

## Vocabulary

| Term       | Meaning                                                                                                                                                                |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sensor     | In-process SDK instance that lives as long as the job (see `principles.md`, 3).                                                                                        |
| Record     | One unit of user data passed to the sensor: any nested structure, or a dataframe row.                                                                                  |
| Rule       | Immutable predicate over records: one expression (`adr/0041`), an action, a sample rate and scope tags. An edit makes a new rule with a new `rule_id` (`adr/0033`).    |
| Expression | One expression in Python-style syntax that gives a boolean, over the fields of a record.                                                                               |
| Action     | `observe` (report only), `drop` (remove the record from the pipeline), `throw` (raise into the pipeline).                                                              |
| Scope tags | Key-value tags on a rule; a sensor syncs only rules matching its own tags.                                                                                             |
| Event      | Report of a match: the record or the matching subset, metadata, and a snapshot of each matched rule as it was evaluated.                                               |
| Metadata   | Key-value context on events. A reserved prefix carries system context (SDK type and version, scheduler and orchestrator identifiers); rest is user-defined.            |
| Operator   | Human managing rules. Roles: admin, operator, observer.                                                                                                                |
| Principal  | Anything that authenticates to the management API: a human, a CLI credential, the AI agent.                                                                            |
| Tenant     | Owner of rules, keys, users and events. A hosted install holds many tenants, a self-hosted install usually one. See `adr/0003`.                                        |
| Superadmin | The user `admin` of the system tenant `$tk`, who creates tenants. See `adr/0028`.                                                                                      |
| Id         | An entity's internal identifier, `<entity>_id`: a time-ordered UUID the server generates. Used in logs and references, never typed or shown to people. See `adr/0020`. |
| Name       | What people type and address an entity by, `<entity>_name`: lowercase, unique in its scope (a tenant's name globally, a user's within its tenant). See `adr/0020`.     |
| Title      | A user-friendly display name, `<entity>_title`, such as `role_title`. Keeps its case; not an identifier. See `adr/0020`.                                               |

## Integration surface

- Sensor SDKs, in order: Rust first, then Python, then Java, then Go. The design
  serves these four (`adr/0038`). Later languages, Node.js and others, are
  undecided, and Node.js comes last. The Rust SDK costs the least to build,
  because the core is written in Rust and the SDK needs no binding to it. Go is
  the hardest case for publishing and native bindings (see `decisions.md`).
  Every SDK gives the same outcome for the same rule and record
  (`principles.md`, 5).
- Framework adapters on top of SDKs: dataframe libraries and cluster compute
  engines (vectorized evaluation), workflow schedulers and container
  orchestrators (automatic metadata).
- Management API, also exposed as an agent tool surface so external agents can
  drive TrapperKeeper.
- Standard operational endpoints: liveness, readiness, metrics.

## Non-goals

- Schema registry, or any server-side understanding of user data.
- Sensor inventory, heartbeats, cross-job sensor identity.
- Guaranteed event delivery. A sensor may lose events on a crash or a network
  partition, because it never blocks or fails the pipeline to deliver them.
- Staged or canary rule rollouts. `observe` plus scope tags plus sampling cover
  the need.
- Pulling data from sources. TrapperKeeper only sees what pipeline code hands
  to a sensor.
