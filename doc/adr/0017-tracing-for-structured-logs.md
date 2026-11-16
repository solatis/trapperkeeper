# 0017. tracing for structured logs

Date: 2026-11-01.

## Context

The milestone M1 asks for structured logs. The format written to standard
output and the source of the log level were open. The owner decided both on
2026-11-01.

## Decision

tracing is the logging facade and tracing-subscriber writes the lines. The
server writes JSON, one object per line, when stdout is not a terminal, and
tracing's compact human format when it is. The server detects the terminal
itself and has no setting for the format. The level is the `log.level` setting
with the default `info`, through the normal precedence of the settings. A set
`RUST_LOG` directive replaces the setting with its full filter syntax. An
invalid directive fails startup.

## Rejected alternatives

JSON only, because a person running the binary by hand would read JSON. A
format setting, because detection covers both cases without one. `RUST_LOG`
only, because the level would escape the precedence every other setting
follows. The `log` crate, because tracing carries spans and fields, and
tower-http emits tracing events.

## Consequences

The same command logs differently under a pipe than in a terminal, and scripts
get JSON. No automated test covers the compact format that the server writes to
a terminal, because only a process shows terminal detection and no test starts a
process (`adr/0018`).
