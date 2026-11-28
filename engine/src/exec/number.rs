//! The loops that compare numbers: a number column with a literal, as in
//! `x > 5`, and two number columns of any types, as in `x < y`.
//!
//! Each comparison is written once, generic over the column types and over
//! the operator. The compiler then builds a separate SIMD loop for each
//! combination that bind asks for. `pack` in `src/column/` runs the loop:
//! it reads 64 rows at a time and turns the answers into one bit per row.

use std::marker::PhantomData;
use std::simd::Simd;

use crate::column::{Number, Pair, Present, Strided, Test, pack};
use crate::plan::Comparison;

/// A comparison operator as a type, such as `Less` for `<`.
///
/// Making the operator a type parameter, instead of a value the loop checks
/// on every row, lets the compiler build one loop per operator with the
/// operator's instruction inlined.
// Each operator is built from just two SIMD tests, `<` and `==`: `x <= y`
// is computed as "not `y < x`". That is wrong for a NaN, where both `<=`
// and `>` are false. It does not matter here, because a NaN counts as a
// row without a value, and `StepResult::compared` throws away the result of
// such rows. On a NaN, `lanes` and `one` may give different answers for the
// same reason.
pub(crate) trait Order {
    /// Compares 64 pairs of values at once and returns one bit per pair, set
    /// where `left op right` is true. Pair `i` is bit `i`.
    fn lanes<T: Number>(left: Simd<T, 64>, right: Simd<T, 64>) -> u64;
    /// Returns `left op right` for one pair of values.
    fn one<T: PartialOrd>(left: T, right: T) -> bool;
}

// One type per operator. `with_order!` picks the type for a `Comparison`.
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

/// Turns a `Comparison`, which is only known at run time, into an operator
/// type that the compiler sees. `with_order!(op, |O| body)` expands to a
/// `match` on `op` with one arm per operator, and each arm compiles `body`
/// with `O` set to that operator's type, such as `Less` for `Lt`. So a loop
/// written once, generic over `O`, gets one compiled copy per operator.
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

/// What `pack` runs to compute `x op literal`, where the literal already has
/// the column's type `T`.
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

/// What `pack` runs to compute `x op literal` for an int column and a float
/// literal: each int converts to `f64` first.
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

/// Computes `x op literal` for the rows `start .. start + rows` of `values`
/// and writes one bit per row into `out`, set where it is true.
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

/// Computes `x op literal` for the rows `start .. start + rows` of an int
/// column and a float literal, converting each int to `f64`, and writes one
/// bit per row into `out`.
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

/// An int literal converted to an int column's type `T`.
///
/// `Within` holds the converted value. `Outside` means that `T` cannot hold
/// the literal, as for 300 and a `u8` column, and says whether the literal is
/// below or above every value of `T`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Converted<T> {
    Within(T),
    Outside(Side),
}

/// Where a literal lies that an int type cannot hold: below its smallest
/// value or above its largest.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Side {
    Below,
    Above,
}

impl<T: TryFrom<i64>> Converted<T> {
    /// Converts `literal` to `T`. When `T` cannot hold it, returns on which
    /// side of `T`'s range it lies.
    pub(crate) fn of(literal: i64) -> Self {
        match T::try_from(literal) {
            Ok(v) => Self::Within(v),
            // A literal that does not fit is below the range when it is
            // negative and above it otherwise, because every int type's range
            // contains 0.
            Err(_) if literal < 0 => Self::Outside(Side::Below),
            Err(_) => Self::Outside(Side::Above),
        }
    }
}

/// Returns the result of `x op literal` for a literal outside the column's
/// range. It is the same for every value of the column: `x < 300` on a `u8`
/// column is true on every row, and `x == 300` false.
pub(crate) fn outside(op: Comparison, side: Side) -> bool {
    let above = matches!(side, Side::Above);
    match op {
        Comparison::Eq => false,
        // No value of the column equals the literal, so `==` is always false
        // and `!=` always true, on either side.
        Comparison::Ne => true,
        Comparison::Lt | Comparison::Le => above,
        Comparison::Gt | Comparison::Ge => !above,
    }
}

/// The type in which two number columns of types `L` and `R` are compared.
///
/// Both columns convert to one type, and the loop compares in that type. The
/// type is chosen so that it holds every value of both columns exactly, or,
/// when an int meets a float, so that the int widens to `f64` as the
/// language says (ADR-0041). Examples:
///
/// - `f16` and `f32` compare as `f32`, so the `f16` side widens.
/// - `i8` and `i32` compare as `i32`, `u8` and `u64` as `u64`.
/// - `i8` and `u8` compare as `i16`, which holds -128 and 255.
/// - `i32` and `u32` compare as `i64`, and `i64` and `u32` also as `i64`.
/// - `i64` and `u64` have no common int type, because no Rust int holds both
///   -1 and `u64::MAX`. See `SignedU64`.
/// - Any int and any float compare as `f64`.
#[derive(Debug, Clone, Copy)]
enum Common {
    // Compare in the type of the left column, or of the right one, because
    // it holds every value of the other.
    Left,
    Right,
    // An int against a float.
    F64,
    // A signed int against an unsigned int, compared in a signed type wide
    // enough for both, such as `i16` for an `i8` against a `u8`.
    I16,
    I32,
    I64,
    // A signed int against a `u64`. A row where the signed value is
    // negative needs no comparison, because a negative number is below
    // every `u64`. The other rows compare as `u64`, which holds both values
    // there. The flag is true when the signed column is on the left.
    SignedU64(bool),
}

/// Picks the type in which a column of `L` and a column of `R` compare (see
/// `Common` for examples). It runs at compile time, once per pair of types.
const fn common<L: Number, R: Number>() -> Common {
    let (l, r) = (size_of::<L>(), size_of::<R>());
    match (L::FLOAT, R::FLOAT) {
        // Two floats: the wider type holds every value of the narrower one.
        (true, true) => wider(l, r),
        // An int and a float: the int widens to `f64` (ADR-0041).
        (true, false) | (false, true) => Common::F64,
        // Two signed or two unsigned ints: the wider type holds every value
        // of the narrower one.
        (false, false) if L::SIGNED == R::SIGNED => wider(l, r),
        (false, false) => {
            // A signed and an unsigned int. A signed type twice as wide as
            // the unsigned one holds all its values: `i16` holds every
            // `u8`. If the signed column is wider still, its own type is
            // enough. Only for a `u64` would twice its width be 16 bytes,
            // and Rust's SIMD has no such type, so that case is `SignedU64`.
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

/// Returns `Left` when the left type, `l` bytes wide, is at least as wide as
/// the right type, `r` bytes wide, and `Right` otherwise.
const fn wider(l: usize, r: usize) -> Common {
    if l >= r { Common::Left } else { Common::Right }
}

/// Returns the result of `left op right` on a row where the signed value is
/// negative and the other value is a `u64`. The negative value is below the
/// `u64`, whatever the two values are. `signed_left` says whether the signed
/// value is `left`.
fn negative_side<O: Order>(signed_left: bool) -> bool {
    // Only the order of the two values matters, so any two values in that
    // order give the answer. `false < true`, so `false` stands in for the
    // negative value and `true` for the `u64`.
    let (negative, unsigned) = (false, true);
    if signed_left {
        O::one(negative, unsigned)
    } else {
        O::one(unsigned, negative)
    }
}

/// What `pack` runs to compute `x op y` for two number columns, converting
/// both sides to the type that `common` picks.
struct Columns<L, R, O> {
    order: PhantomData<(L, R, O)>,
}

impl<L: Number, R: Number, O: Order> Test<Pair<'_, L, R>> for Columns<L, R, O> {
    fn lanes(&self, (left, right): (Simd<L, 64>, Simd<R, 64>)) -> u64 {
        // `const` makes the compiler evaluate `common` once per pair of types,
        // so in each compiled loop the other arms are dead code that the
        // optimizer removes.
        match const { common::<L, R>() } {
            // The left type holds every right value: convert the right side.
            Common::Left => O::lanes(left, R::convert_lanes(right)),
            // The right type holds every left value: convert the left side.
            Common::Right => O::lanes(L::convert_lanes(left), right),
            // Neither type holds the other: convert both sides to a third
            // type.
            Common::F64 => in_type::<f64, L, R, O>(left, right),
            Common::I16 => in_type::<i16, L, R, O>(left, right),
            Common::I32 => in_type::<i32, L, R, O>(left, right),
            Common::I64 => in_type::<i64, L, R, O>(left, right),
            // A signed int against a `u64`. The loop computes two answers for
            // all 64 rows, and then picks per row: the fixed answer for the
            // rows with a negative value, and the `u64` comparison for the
            // rest. Picking with bit masks avoids a branch per row.
            Common::SignedU64(signed_left) => {
                // One bit per row, set where the signed value is below 0.
                let negative = if signed_left {
                    L::lt_lanes(left, Simd::splat(L::default()))
                } else {
                    R::lt_lanes(right, Simd::splat(R::default()))
                };
                // The answer on a negative row is the same for every such
                // row, so it is all ones or all zeros.
                let below = if negative_side::<O>(signed_left) {
                    u64::MAX
                } else {
                    0
                };
                // The comparison as `u64`. It is wrong on the negative rows,
                // where a negative value wraps to a large `u64`, and the mask
                // below discards it there.
                let rest = in_type::<u64, L, R, O>(left, right);
                negative & below | !negative & rest
            }
        }
    }

    fn one(&self, (left, right): (L, R)) -> bool {
        // The same choice as `lanes`, for one pair of values. `pack` uses
        // this for the rows after the last full group of 64, and for columns
        // whose values are not next to each other in memory.
        match const { common::<L, R>() } {
            Common::Left => O::one(left, right.convert()),
            Common::Right => O::one(left.convert(), right),
            Common::F64 => O::one::<f64>(left.convert(), right.convert()),
            Common::I16 => O::one::<i16>(left.convert(), right.convert()),
            Common::I32 => O::one::<i32>(left.convert(), right.convert()),
            Common::I64 => O::one::<i64>(left.convert(), right.convert()),
            Common::SignedU64(signed_left) => {
                // Is the signed value below 0?
                let negative = if signed_left {
                    left < L::default()
                } else {
                    right < R::default()
                };
                // A negative value is below every `u64`, so the answer is
                // fixed. Otherwise both values fit in a `u64`.
                if negative {
                    negative_side::<O>(signed_left)
                } else {
                    O::one::<u64>(left.convert(), right.convert())
                }
            }
        }
    }
}

/// Converts 64 values of `L` and 64 values of `R` to `C`, compares them
/// pairwise with `O`, and returns one bit per pair.
fn in_type<C: Number, L: Number, R: Number, O: Order>(
    left: Simd<L, 64>,
    right: Simd<R, 64>,
) -> u64 {
    O::lanes(L::convert_lanes::<C>(left), R::convert_lanes::<C>(right))
}

/// Computes `x op y` row by row for the rows `start .. start + rows` of the
/// columns `left` and `right`, and writes one bit per row into `out`, set
/// where it is true. Both values of a row convert to the type that `common`
/// picks.
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
