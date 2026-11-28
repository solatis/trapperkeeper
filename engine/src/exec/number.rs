//! The comparisons of number columns with a literal and with another number
//! column of any type. Each comparison is a `Test` of `pack`, which the
//! compiler instantiates per type and per operator.

use std::marker::PhantomData;
use std::simd::Simd;

use crate::column::{Number, Pair, Present, Strided, Test, pack};
use crate::plan::Comparison;

/// One of the six comparisons, as a type, so that each loop is compiled for
/// its operator.
// `StepResult::compared` clears the result of a row without a value, such as a
// NaN, so `<=` can be the negation of `>` and the impls need only the two lane
// tests of `Number`. On a NaN, `lanes` and `one` then answer differently.
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
            $crate::plan::Comparison::Eq => {
                type $O = $crate::exec::number::Equal;
                $body
            }
            $crate::plan::Comparison::Ne => {
                type $O = $crate::exec::number::NotEqual;
                $body
            }
            $crate::plan::Comparison::Lt => {
                type $O = $crate::exec::number::Less;
                $body
            }
            $crate::plan::Comparison::Le => {
                type $O = $crate::exec::number::LessEqual;
                $body
            }
            $crate::plan::Comparison::Gt => {
                type $O = $crate::exec::number::Greater;
                $body
            }
            $crate::plan::Comparison::Ge => {
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

/// The value of an int column's type that a literal converts to, or the side
/// of the type where the literal lies when the type cannot hold it.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Converted<T> {
    Within(T),
    Outside(Side),
}

/// The side of an int type where a literal that the type cannot hold lies.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Side {
    Below,
    Above,
}

impl<T: TryFrom<i64>> Converted<T> {
    /// Converts `literal` to `T`, or says on which side of the type it lies
    /// when `T` cannot hold it.
    pub(crate) fn of(literal: i64) -> Self {
        match T::try_from(literal) {
            Ok(v) => Self::Within(v),
            // A literal that fails to convert lies below the type when it is
            // negative, because every int type holds 0.
            Err(_) if literal < 0 => Self::Outside(Side::Below),
            Err(_) => Self::Outside(Side::Above),
        }
    }
}

/// The result of `column op literal` on every row with a value, for a
/// literal that lies on `side` of the column's type.
pub(crate) fn outside(op: Comparison, side: Side) -> bool {
    let above = matches!(side, Side::Above);
    match op {
        Comparison::Eq => false,
        // No value of the column equals a literal outside its type, so `!=`
        // holds on both sides.
        Comparison::Ne => true,
        Comparison::Lt | Comparison::Le => above,
        Comparison::Gt | Comparison::Ge => !above,
    }
}

/// The type in which two number columns compare, which the pair of their types
/// decides.
#[derive(Debug, Clone, Copy)]
enum Common {
    // The type of the left column, or of the right one, holds every value of
    // the other.
    Left,
    Right,
    F64,
    // A signed type wider than both, for a signed and an unsigned int, such
    // as `i16` for an `i8` against a `u8`.
    I16,
    I32,
    I64,
    // A signed int against a `u64`, which no int type holds both of. `true`
    // when the signed int is on the left.
    SignedU64(bool),
}

/// The common type of a column of `L` and a column of `R`.
const fn common<L: Number, R: Number>() -> Common {
    let (l, r) = (size_of::<L>(), size_of::<R>());
    match (L::FLOAT, R::FLOAT) {
        // The wider float type holds every value of the narrower one.
        (true, true) => wider(l, r),
        // An int widens to a float64 (ADR-0041).
        (true, false) | (false, true) => Common::F64,
        // The wider int type of one signedness holds every value of the
        // narrower one.
        (false, false) if L::SIGNED == R::SIGNED => wider(l, r),
        (false, false) => {
            // A signed type twice as wide as an unsigned one holds every value
            // of it. The signed type of the pair may already be wider, and
            // then it holds both. No int type holds every value of a `u64`
            // and of a signed type.
            let (signed, unsigned) = if L::SIGNED { (l, r) } else { (r, l) };
            let width = if 2 * unsigned > signed {
                2 * unsigned
            } else {
                signed
            };
            match width {
                2 => Common::I16,
                4 => Common::I32,
                8 => Common::I64,
                _ => Common::SignedU64(L::SIGNED),
            }
        }
    }
}

/// The side of a pair whose type is `l` bytes wide on the left and `r` on the
/// right, whichever is wider.
const fn wider(l: usize, r: usize) -> Common {
    if l >= r { Common::Left } else { Common::Right }
}

/// The result of `O` for a row whose signed value is negative. The signed
/// value is the left operand when `signed_left` is true, and the right one
/// otherwise. A negative value orders below every `u64`.
fn negative_side<O: Order>(signed_left: bool) -> bool {
    // The booleans stand in for the two values, because `false` orders below
    // `true` as the negative value orders below the `u64`.
    let (negative, unsigned) = (false, true);
    if signed_left {
        O::one(negative, unsigned)
    } else {
        O::one(unsigned, negative)
    }
}

/// The test of `pack` that compares the values of two number columns in the
/// common type of their pair.
struct Columns<L, R, O> {
    order: PhantomData<(L, R, O)>,
}

impl<L: Number, R: Number, O: Order> Test<Pair<'_, L, R>> for Columns<L, R, O> {
    fn lanes(&self, (left, right): (Simd<L, 64>, Simd<R, 64>)) -> u64 {
        match const { common::<L, R>() } {
            Common::Left => O::lanes(left, R::convert_lanes(right)),
            Common::Right => O::lanes(L::convert_lanes(left), right),
            Common::F64 => in_type::<f64, L, R, O>(left, right),
            Common::I16 => in_type::<i16, L, R, O>(left, right),
            Common::I32 => in_type::<i32, L, R, O>(left, right),
            Common::I64 => in_type::<i64, L, R, O>(left, right),
            Common::SignedU64(signed_left) => {
                let negative = if signed_left {
                    L::lt_lanes(left, Simd::splat(L::default()))
                } else {
                    R::lt_lanes(right, Simd::splat(R::default()))
                };
                // The order decides a row with a negative value, and the
                // other rows compare as `u64`.
                let below = if negative_side::<O>(signed_left) {
                    u64::MAX
                } else {
                    0
                };
                let rest = in_type::<u64, L, R, O>(left, right);
                negative & below | !negative & rest
            }
        }
    }

    fn one(&self, (left, right): (L, R)) -> bool {
        match const { common::<L, R>() } {
            Common::Left => O::one(left, right.convert()),
            Common::Right => O::one(left.convert(), right),
            Common::F64 => O::one::<f64>(left.convert(), right.convert()),
            Common::I16 => O::one::<i16>(left.convert(), right.convert()),
            Common::I32 => O::one::<i32>(left.convert(), right.convert()),
            Common::I64 => O::one::<i64>(left.convert(), right.convert()),
            Common::SignedU64(signed_left) => {
                let negative = if signed_left {
                    left < L::default()
                } else {
                    right < R::default()
                };
                if negative {
                    negative_side::<O>(signed_left)
                } else {
                    O::one::<u64>(left.convert(), right.convert())
                }
            }
        }
    }
}

/// The comparison of 64 lanes of `L` with 64 lanes of `R`, both converted to
/// `C`.
fn in_type<C: Number, L: Number, R: Number, O: Order>(
    left: Simd<L, 64>,
    right: Simd<R, 64>,
) -> u64 {
    O::lanes(L::convert_lanes::<C>(left), R::convert_lanes::<C>(right))
}

/// Writes into `out` whether each of the rows `start .. start + rows` of
/// `left` compares with the same row of `right` as `O` says, both converted to
/// the common type of their pair.
///
/// # Panics
///
/// Panics when the two columns hold different numbers of values, when `rows`
/// is above `CHUNK_ROWS` or when the chunk reaches past the columns.
pub(crate) fn compare_columns<L: Number, R: Number, O: Order>(
    left: Strided<'_, L>,
    right: Strided<'_, R>,
    start: usize,
    rows: usize,
    out: &mut Present,
) {
    let test = Columns::<L, R, O> { order: PhantomData };
    pack(Pair::new(left, right), start, rows, out, &test);
}
