# 0002. AI agent is an external client, and manages rules only

Date: 2026-11-15.

## Context

An agent that troubleshoots data problems needs to read events and change
rules. Running the agent inside the API server would couple model latency,
cost and prompt-injection exposure to the server. It would also create a call
cycle, in which the server launches the agent and the agent calls the server.

## Decision

The agent is a separate component and a client of the management API. It has
its own top-level directory, `agent/`, and it talks to the management API
through tool calls (owner, 2026-11-15).

The agent manages rules and does not manage sensors. For a new instrumentation
point in pipeline code, the agent at most suggests a patch to a human.

The agent may create rules of any kind, with any action (owner, 2026-11-15). Two
questions are open (`decisions.md`): how a rule records that the agent wrote it,
and how the agent signs in.

## Rejected alternatives

The agent inside the API server, for the latency, cost, exposure and call
cycle above.

## Consequences

The agent gets the same checks as a human, can be replaced by another agent that
calls the same API, and is optional to deploy. The agent component cites this
record in its `AGENTS.md` when its directory is created with its first code.
