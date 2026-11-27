# 0045. amd64 builds target x86-64-v3

Date: 2026-11-27.

## Context

The loops over values that lie one after the other use `std::simd`
(`adr/0035`). `std::simd` picks its instructions when the code compiles, for
the target features that the build turns on, and it checks no CPU feature at
run time. A build for the baseline amd64 target therefore runs SSE2 on every
CPU, also on a CPU that has AVX2.

A half-precision float shows the cost most clearly. On rustc 1.102.0-nightly
of 2026-11-25, one comparison of 64 `f16` lanes compiled for the baseline
amd64 target holds 129 calls to `__extendhfsf2`, one conversion per lane. For
`x86-64-v3` it holds `vcvtph2ps` and no call. The aarch64 targets need no
change: macOS compares `f16` lanes natively with `fcmgt.8h`, and the Linux
baseline widens them with `fcvtl` and makes no call.

## Decision

Every amd64 build targets `x86-64-v3`, which brings AVX2, FMA, F16C and BMI2
(owner, 2026-11-27). The level is old enough to require, since Intel's Haswell
of 2013 has it (owner, 2026-11-27). `.cargo/config.toml` sets
`-C target-cpu=x86-64-v3` for every target whose architecture is `x86_64`.

A `RUSTFLAGS` in the environment replaces the flags of `.cargo/config.toml`
instead of adding to them. The engine crate therefore refuses to compile for
amd64 when AVX2 or F16C is missing, so a build that lost the flag fails
instead of running SSE2 loops.

The aarch64 targets keep their defaults.

## Rejected alternatives

The baseline amd64 target, because it runs no AVX2 in any loop and converts
each `f16` lane through a call.

Copies of the loops compiled with `#[target_feature]`, chosen once at run time
with `is_x86_feature_detected!`, because they double the compiled loops and add
a check to each call, for CPUs older than 2013.

## Consequences

The engine does not run on an amd64 CPU older than `x86-64-v3`. The Python
wheels of the milestone "Python sensor SDK" inherit that requirement.

A shell or a CI job that sets `RUSTFLAGS` adds `-C target-cpu=x86-64-v3` to it
for an amd64 build.

Miri runs the engine's tests on Linux amd64 (`.github/workflows/miri.yml`),
and it reads `.cargo/config.toml` like any other build.
