//! The comparisons of number columns with a literal. Each comparison is a
//! `Test` of `pack`, which the compiler instantiates per type and per
//! operator.

use std::marker::PhantomData;
use std::simd::Simd;

use crate::column::{Number, Present, Strided, Test, pack};
use crate::plan::Comparison;

/// One of the six comparisons, as a type, so that each loop is compiled for
/// its operator.
pub(crate) trait Order {
    /// For 64 lanes, whether the comparison holds, with lane `i` in bit `i`.
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64;
    /// Whether the comparison holds for one pair of values.
    fn one<T: PartialOrd>(left: T, right: T) -> bool;
}

// The orders as types. `with_order!` picks one from a `Comparison`.
pub(crate) struct Equal;
pub(crate) struct NotEqual;
pub(crate) struct Less;
pub(crate) struct LessEqual;
pub(crate) struct Greater;
pub(crate) struct GreaterEqual;

// A NaN is no value (ADR-0005), and a lane without a value is unknown
// whatever the loop gives for it. So `<=` can be the negation of `>`, and the
// orders need only the two lane tests of `Number`.
impl Order for Equal {
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64 {
        T::eq_lanes(left, right)
    }

    fn one<T: PartialOrd>(left: T, right: T) -> bool {
        left == right
    }
}

impl Order for NotEqual {
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64 {
        !T::eq_lanes(left, right)
    }

    fn one<T: PartialOrd>(left: T, right: T) -> bool {
        left != right
    }
}

impl Order for Less {
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64 {
        T::lt_lanes(left, right)
    }

    fn one<T: PartialOrd>(left: T, right: T) -> bool {
        left < right
    }
}

impl Order for LessEqual {
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64 {
        !T::lt_lanes(right, left)
    }

    fn one<T: PartialOrd>(left: T, right: T) -> bool {
        left <= right
    }
}

impl Order for Greater {
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64 {
        T::lt_lanes(right, left)
    }

    fn one<T: PartialOrd>(left: T, right: T) -> bool {
        left > right
    }
}

impl Order for GreaterEqual {
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64 {
        !T::lt_lanes(left, right)
    }

    fn one<T: PartialOrd>(left: T, right: T) -> bool {
        left >= right
    }
}

/// Runs `$body` with `$O` as the type of the order `$op`.
macro_rules! with_order {
    ($op:expr, |$O:ident| $body:expr) => {
        match $op {
            Comparison::Eq => {
                type $O = $crate::exec::number::Equal;
                $body
            }
            Comparison::Ne => {
                type $O = $crate::exec::number::NotEqual;
                $body
            }
            Comparison::Lt => {
                type $O = $crate::exec::number::Less;
                $body
            }
            Comparison::Le => {
                type $O = $crate::exec::number::LessEqual;
                $body
            }
            Comparison::Gt => {
                type $O = $crate::exec::number::Greater;
                $body
            }
            Comparison::Ge => {
                type $O = $crate::exec::number::GreaterEqual;
                $body
            }
        }
    };
}
pub(crate) use with_order;

/// The test of `pack` that compares each value with a literal of its own type.
struct AgainstLiteral<T, O> {
    literal: T,
    order: PhantomData<O>,
}

impl<T: Number, O: Order> Test<Strided<'_, T>> for AgainstLiteral<T, O> {
    fn lanes(&self, values: Simd<T, 64>) -> u64 {
        O::lanes(values, Simd::splat(self.literal))
    }

    fn one(&self, value: T) -> bool {
        O::one(value, self.literal)
    }
}

/// The test of `pack` that widens each int to `f64` and compares it with a
/// float literal.
struct AgainstFloat<O> {
    literal: f64,
    order: PhantomData<O>,
}

impl<T: Number, O: Order> Test<Strided<'_, T>> for AgainstFloat<O> {
    fn lanes(&self, values: Simd<T, 64>) -> u64 {
        O::lanes(T::convert_lanes::<f64>(values), Simd::splat(self.literal))
    }

    fn one(&self, value: T) -> bool {
        O::one(value.convert::<f64>(), self.literal)
    }
}

/// Writes into `out` whether each of the rows `start .. start + rows` of
/// `values` compares with `literal` as `O` says.
pub(crate) fn compare_with_literal<T: Number, O: Order>(
    values: Strided<'_, T>,
    literal: T,
    start: usize,
    rows: usize,
    out: &mut Present,
) {
    let test = AgainstLiteral::<T, O> {
        literal,
        order: PhantomData,
    };
    pack(values, start, rows, out, &test);
}

/// Writes into `out` whether each of the rows `start .. start + rows` of an
/// int column, widened to `f64`, compares with a float literal as `O` says.
pub(crate) fn compare_widened<T: Number, O: Order>(
    values: Strided<'_, T>,
    literal: f64,
    start: usize,
    rows: usize,
    out: &mut Present,
) {
    let test = AgainstFloat::<O> {
        literal,
        order: PhantomData,
    };
    pack(values, start, rows, out, &test);
}

/// The value of an int column's type that a literal converts to, or where
/// the literal lies when the type cannot hold it.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Converted<T> {
    Within(T),
    Below,
    Above,
}

impl<T: TryFrom<i64>> Converted<T> {
    /// `literal` in the type `T`.
    pub(crate) fn of(literal: i64) -> Self {
        match T::try_from(literal) {
            Ok(v) => Self::Within(v),
            // A literal that fails to convert lies below the type when it is
            // negative, because every int type holds 0.
            Err(_) if literal < 0 => Self::Below,
            Err(_) => Self::Above,
        }
    }
}

/// The result, on every row with a value, of a comparison whose literal lies
/// below the column's type, or above it when `above`.
pub(crate) fn outside(op: Comparison, above: bool) -> bool {
    match op {
        Comparison::Eq => false,
        // No value of the column equals a literal outside its type, so `!=`
        // holds on both sides.
        Comparison::Ne => true,
        Comparison::Lt | Comparison::Le => above,
        Comparison::Gt | Comparison::Ge => !above,
    }
}
