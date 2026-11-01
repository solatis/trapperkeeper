# 0012. clap and figment for the command line and configuration

Date: 2026-11-01.

## Context

The server binary needs subcommands and a precedence rule for its settings.
The milestone fixes the precedence: a command line flag beats an environment
variable, which beats the configuration file. How the layers are merged was
open.

## Decision

clap, through its derive API, parses the command line, so the command line is
one annotated struct. figment merges the settings layers into one serde
struct, lowest layer first: the defaults, the TOML file named by `--config`,
the environment variables prefixed `TRAPPERKEEPER_`, and the parsed flags. The
precedence is the order of the merge calls, a declaration rather than merge
code.

## Rejected alternatives

A hand-written merge, serde for the file and then overwriting from the
environment and the flags, because every new setting needs three lines of
merge code and the precedence lives in that code. The `config` crate, because
its values stay untyped until the final deserialize and its environment
mapping is less direct than figment's.

## Consequences

The flags struct must serialize to the same tree as the settings struct, and
an absent flag must serialize to nothing, or it would overwrite the layer
below. figment emits an empty dictionary for a file that does not exist, so
the loader checks that a named file is present itself.
