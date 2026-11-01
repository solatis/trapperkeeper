# 0017. tracing for structured logs

Date: 2026-11-01.

## Context

The milestone asks for structured logs. The format written to standard output
and the source of the log level were open; the owner decided both on
2026-11-01.

## Decision

tracing is the logging facade and tracing-subscriber writes the lines. The
output is JSON, one object per line, when stdout is not a terminal, and
tracing's compact human format when it is; the server detects the terminal
and has no setting for the format. The level is the `log.level` setting with
the default `info`, through the normal precedence of the settings. A `RUST_LOG`
directive, when the variable is set, replaces the setting with its full filter
syntax, and an invalid directive fails startup.

## Rejected alternatives

JSON only, because a person running the binary by hand would read JSON. A
format setting, because detection covers both cases without one. `RUST_LOG`
only, because the level would escape the precedence every other setting
follows. The `log` crate, because tracing carries spans and fields, and
tower-http emits tracing events.

## Consequences

The same command logs differently under a pipe than in a terminal, and scripts
get JSON. The readable branch has no automated test, because CI has no
terminal.
