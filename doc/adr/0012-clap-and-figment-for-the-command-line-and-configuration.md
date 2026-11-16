# 0012. clap and figment for the command line and configuration

Date: 2026-11-01.

## Context

The server binary needs subcommands and a precedence rule for its settings.
The milestone M1 fixes the precedence. A command line flag beats an
environment variable, and an environment variable beats the configuration
file. How the layers are merged was open.

## Decision

clap, through its derive API, parses the command line, so the command line is
one annotated struct. figment merges the settings layers into one serde
struct, lowest layer first: the defaults, the TOML file named by `--config`,
the environment variables prefixed `TRAPPERKEEPER_`, and the parsed flags. The
order of the merge calls in `config::load` sets the precedence, so no merge
code states it.

## Rejected alternatives

A hand-written merge, serde for the file and then overwriting from the
environment and the flags, because every new setting needs three lines of
merge code and the precedence lives in that code. The `config` crate, because
its values stay untyped until the final deserialize, and it maps the
environment less directly than figment does.

## Consequences

The flags struct must serialize to the same tree as the settings struct, and an
absent flag must serialize to nothing, or it would overwrite the layer below.
figment reads a missing file as an empty dictionary, so `config::load` itself
checks that a named configuration file exists.
