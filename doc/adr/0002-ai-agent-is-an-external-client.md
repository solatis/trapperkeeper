# 0002. AI agent is an external client, and manages rules only

Date: 2026-09-21.

## Context

An agent that troubleshoots data problems needs to read events and change
rules. Running it inside the API server would couple model latency, cost and
prompt-injection exposure to the server, and would create a call cycle: the
server launches the agent, and the agent calls the server.

## Decision

The agent is a separate component and a client of the management API with its
own principal and role. It manages rules, not sensors: a new instrumentation
point in pipeline code is at most a suggested patch for a human. Agent-authored
rules start as `observe` and are tagged as agent-authored; promotion to `drop`
or `throw` needs a human.

## Rejected alternatives

The agent inside the API server, for the latency, cost, exposure and call
cycle above.

## Consequences

The agent gets the same checks as a human, can be swapped for an external
agent, and is optional to deploy. The agent component cites this record.
