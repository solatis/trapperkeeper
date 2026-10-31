# Overview

## The problem

Data pipelines run on ephemeral compute: scheduler tasks, cluster jobs,
containers, serverless functions. The data flowing through them changes shape
constantly. Teams want quality checks and visibility on that data, but existing
options force a bad trade:

- checks hardcoded in pipeline code need a redeploy for every change;
- schema-registry style tools need upfront registration and break on drift;
- a monitoring tool that can fail the pipeline becomes a production risk, so it
  gets removed.

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
- The **AI agent** troubleshoots data problems on request: it queries events,
  and creates, scopes, samples and pauses rules on its own, inside guardrails.

## Users

- Data engineers: embed the sensor, a handful of lines per pipeline.
- Operators / data-quality owners: author rules, watch events.
- Platform teams: deploy and run the server.

## Vocabulary

| Term       | Meaning                                                                                                                                                     |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sensor     | In-process SDK instance, lives as long as the job. No registration, no identity, no server-side inventory.                                                  |
| Record     | One unit of user data passed to the sensor: any nested structure, or a dataframe row.                                                                       |
| Rule       | Immutable, versioned predicate over records plus an action, a sample rate and scope tags.                                                                   |
| Condition  | Field path, declared field type, operator, value, and two policies: what to do on a missing field, what to do on a type coercion failure.                   |
| Rule shape | OR of groups, each group an AND of conditions. Maps directly to a visual rule builder.                                                                      |
| Field path | List of segments (key, index, wildcard) resolved at evaluation time. A wildcard matches if any element matches.                                             |
| Action     | `observe` (report only), `drop` (remove the record from the pipeline), `fail` (raise into the pipeline).                                                    |
| Scope tags | Key-value tags on a rule; a sensor syncs only rules matching its own tags.                                                                                  |
| Event      | Report of a match: the record or the matching subset, metadata, and a snapshot of each matched rule as it was evaluated.                                    |
| Metadata   | Key-value context on events. A reserved prefix carries system context (SDK type and version, scheduler and orchestrator identifiers); rest is user-defined. |
| Operator   | Human managing rules. Roles: admin, operator, observer.                                                                                                     |
| Principal  | Anything that authenticates to the management API: a human, a CLI credential, the AI agent.                                                                 |
| Tenant     | Owner of rules, keys, users and events. See `decisions.md`.                                                                                                 |

## Integration surface

- Sensor SDKs: Python, Java, Go and Rust first; Node.js and further languages
  on demand. Go is in the first set as a stress test: it is the hardest case
  for publishing and native bindings (see `decisions.md`). Rust is expected to
  be cheap, as the server side is written in Rust. All SDKs must produce
  identical match results for identical input.
- Framework adapters on top of SDKs: dataframe libraries and cluster compute
  engines (vectorized evaluation), workflow schedulers and container
  orchestrators (automatic metadata).
- Management API, also exposed as an agent tool surface so external agents can
  drive TrapperKeeper.
- Standard operational endpoints: liveness, readiness, metrics.

## Non-goals

- Schema registry, or any server-side understanding of user data.
- Sensor inventory, heartbeats, cross-job sensor identity.
- Guaranteed event delivery. Events may be lost on crash or network partition;
  the pipeline comes first.
- Staged or canary rule rollouts. `observe` plus scope tags plus sampling cover
  the need.
- Pulling data from sources. TrapperKeeper only sees what pipeline code hands
  to a sensor.
