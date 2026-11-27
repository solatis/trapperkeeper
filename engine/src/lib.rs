//! The rule engine. It holds the rule and the tree of its expression with
//! their JSON form, parses and prints a rule's text, and validates a rule. It
//! also evaluates rules. `plan` turns a rule set into a program, `exec` binds
//! the program to the columns of a schema and runs it over a batch, and
//! `column` describes those columns.

// The half-precision float of numpy and pandas columns is nightly's `f16`,
// and ADR-0036 names this feature, as ADR-0009 asks.
#![feature(f16)]
// The loops over values that lie one after the other use `std::simd`, and
// ADR-0035 names this feature.
#![feature(portable_simd)]

// An amd64 build without AVX2 and F16C means that a RUSTFLAGS in the
// environment replaced the target CPU that `.cargo/config.toml` sets
// (ADR-0045). Such a build would run SSE2 loops with nothing to show it, so
// it fails here.
#[cfg(all(
    target_arch = "x86_64",
    not(all(target_feature = "avx2", target_feature = "f16c"))
))]
compile_error!("amd64 builds target x86-64-v3: add `-C target-cpu=x86-64-v3` to RUSTFLAGS");

// The crate root exports nothing of `column`, because its names, such as
// `Strided`, are general and read better as `column::Strided`.
pub mod column;
pub mod exec;
pub mod expr;
pub mod limits;
pub mod plan;
pub mod rule;
pub mod text;
pub mod validate;

pub use expr::{Expr, Segment, TypeName};
pub use rule::{Action, Rule};
// `text::Reason` stays in its module, because `validate::Reason` has that
// name here.
pub use text::{SyntaxError, parse};
pub use validate::{Error, Reason, validate};
