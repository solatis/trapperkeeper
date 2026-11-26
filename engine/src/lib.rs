//! The rule model: what a rule is, the tree of its expression, their JSON
//! form, the parser and the printer of a rule's text, and the validation of
//! a rule. The crate also holds the description of a column, over which the
//! executor will run. No code of this crate evaluates a rule yet.

// The half-precision float of numpy and pandas columns is nightly's `f16`,
// and ADR-0036 names this feature, as ADR-0009 asks.
#![feature(f16)]
// The contiguous loops use `std::simd`, and ADR-0035 names this feature.
#![feature(portable_simd)]

// The crate root exports nothing of `column`, because its names, such as
// `Strided`, are general and read better as `column::Strided`.
pub mod column;
pub mod expr;
pub mod limits;
pub mod rule;
pub mod text;
pub mod validate;

pub use expr::{Expr, Segment, TypeName};
pub use rule::{Action, Rule};
// `text::Reason` stays in its module, because `validate::Reason` has that
// name here.
pub use text::{SyntaxError, parse};
pub use validate::{Error, Reason, validate};
