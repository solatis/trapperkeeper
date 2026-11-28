//! The sets of a membership step, built once at bind for the column's type,
//! and the tests that look a row up in them. A set holds the literals of a
//! list such as `x in [1, 2, 3]`, so one lookup per row gives what
//! `x == 1 or x == 2 or x == 3` gives.

use std::cmp::Ordering;
use std::simd::Simd;

use super::bind::{
    BoundStep, ChunkLoop, FieldType, booleans_equal, constant_loop, convert, values_of,
};
use super::number::Converted;
use super::text::{Encoded, texts_of};
use crate::column::{
    Column, Number, Present, Test, TextRows, Texts, pack, with_float, with_int, with_texts,
};
use crate::plan::{Constant, Literal};

/// The most values for which a number set compares a vector of 64 rows with
/// each value. Above it, each row takes a binary search.
// No measurement chose 8.
const SPLAT_LIMIT: usize = 8;

/// Builds the step `x in values` for field `f`, whose column has type `t`.
/// The step is true where the value equals one of `values`, false on the
/// other rows with a value, and unknown on the rows without one.
pub(super) fn bind_member(f: usize, t: FieldType, values: &[Literal]) -> BoundStep {
    // The step gives what `x == a or x == b ...` gives for its values, with
    // one lookup per row in a set of the column's type.
    //
    // 1. A field that the source lacks gives unknown on every row (ADR-0005).
    // 2. It drops the values whose type can never equal the column's,
    //    because their equalities are false and add nothing to the `or`.
    // 3. It converts the other values to the column's type into a set, as
    //    `bind_literal` converts the literal of `==`, and builds the loop that
    //    looks each row up in the set. With nothing left in the set, the loop
    //    is false on every row.

    // 1. a missing field
    if t == FieldType::Absent {
        return BoundStep::Constant(Constant::Unknown);
    }

    // 2. drop the values that can never be equal
    let kept: Vec<&Literal> = values.iter().filter(|v| can_equal(t, v)).collect();

    // 3. build the set and its loop
    let chunk = match t {
        // `with_float!` and `with_int!` expand into a match with one arm per
        // number type, and each arm sets `T` to that type, so the set and its
        // loop are compiled for the column's type.
        FieldType::Number(n) if n.is_float() => {
            with_float!(n, |T| number_loop(f, NumberSet::<T>::for_float(&kept)))
        }
        FieldType::Number(n) => with_int!(n, |T| number_loop(f, NumberSet::<T>::for_int(&kept))),
        FieldType::Bool => flags_loop(f, &kept),
        FieldType::Text => text_loop(f, TextSet::of(&kept)),
        // No literal can equal a list, so the filter kept none, and a missing
        // field returned before the filter.
        FieldType::List(_) | FieldType::Absent => constant_loop(false),
    };
    BoundStep::Loop {
        fields: vec![f],
        chunk,
    }
}

/// Whether a value of column type `t` can ever equal `literal`: a number
/// equals a number, and a boolean or a text equals only its own kind.
fn can_equal(t: FieldType, literal: &Literal) -> bool {
    matches!(
        (t, literal),
        (FieldType::Number(_), Literal::Int(_) | Literal::Float(_))
            | (FieldType::Bool, Literal::Bool(_))
            | (FieldType::Text, Literal::Text(_))
    )
}

/// The values of a membership step that a number column of type `T` can
/// equal. `values` holds them in `T`. On an int column, `widened` holds the
/// float literals, which compare with the row widened to `f64`, as in
/// `x == 2.0` (ADR-0041).
pub(super) struct NumberSet<T> {
    values: Vec<T>,
    widened: Vec<f64>,
}

impl<T: Number + TryFrom<i64>> NumberSet<T> {
    /// Builds the set of an int column of type `T`. An int literal that `T`
    /// cannot hold equals no row and is dropped.
    pub(super) fn for_int(kept: &[&Literal]) -> Self {
        let mut values = Vec::new();
        let mut widened = Vec::new();
        for literal in kept {
            match **literal {
                Literal::Int(i) => {
                    if let Converted::Within(v) = Converted::<T>::of(i) {
                        values.push(v);
                    }
                }
                Literal::Float(x) => widened.push(x),
                Literal::Bool(_) | Literal::Text(_) => {
                    unreachable!("the caller keeps only numbers for a number column")
                }
            }
        }
        Self {
            values: sorted(values),
            widened: sorted(widened),
        }
    }
}

impl<T: Number> NumberSet<T> {
    /// Builds the set of a float column of type `T`. Each literal rounds to
    /// the nearest value of `T`, as the literal of `x == 0.1` does
    /// (ADR-0041).
    pub(super) fn for_float(kept: &[&Literal]) -> Self {
        let values = kept
            .iter()
            .map(|literal| match **literal {
                Literal::Int(i) => convert::<i64, T>(i),
                Literal::Float(x) => convert::<f64, T>(x),
                Literal::Bool(_) | Literal::Text(_) => {
                    unreachable!("the caller keeps only numbers for a number column")
                }
            })
            .collect();
        Self {
            values: sorted(values),
            widened: Vec::new(),
        }
    }

    /// Whether no row can be in the set.
    pub(super) fn is_empty(&self) -> bool {
        self.values.is_empty() && self.widened.is_empty()
    }
}

/// Returns `values` sorted and without repeats, for a binary search.
// `-0.0` and `0.0` compare equal under `partial_cmp`, `dedup` and the vector
// compare alike, so the set keeps one of them and a row of either sign finds
// it.
fn sorted<U: Number>(mut values: Vec<U>) -> Vec<U> {
    // A literal is never NaN, because JSON cannot write one, so every pair
    // has an order.
    values.sort_by(|a, b| a.partial_cmp(b).expect("a literal is not NaN"));
    values.dedup();
    values
}

/// Whether the sorted set `set` holds `v`.
fn contains<U: Number>(set: &[U], v: U) -> bool {
    // A NaN row has no order. It is a row without a value, whose result
    // `StepResult::compared` throws away, so any answer serves.
    set.binary_search_by(|x| x.partial_cmp(&v).unwrap_or(Ordering::Less))
        .is_ok()
}

/// Returns one bit per lane of `lanes`, set where the lane's value is in the
/// sorted set `set`.
fn lanes_in<U: Number>(set: &[U], lanes: Simd<U, 64>) -> u64 {
    // A short set compares all 64 lanes with each of its values, which costs
    // a few vector compares. A longer set looks each lane up by binary
    // search, which costs one search per lane and grows only with the log of
    // the set's length.
    if set.len() <= SPLAT_LIMIT {
        set.iter()
            .fold(0, |hits, &x| hits | U::eq_lanes(lanes, Simd::splat(x)))
    } else {
        lanes
            .to_array()
            .iter()
            .enumerate()
            .fold(0, |hits, (i, &v)| hits | (u64::from(contains(set, v)) << i))
    }
}

impl<T: Number> Test<crate::column::Strided<'_, T>> for NumberSet<T> {
    fn lanes(&self, lanes: Simd<T, 64>) -> u64 {
        let mut hits = lanes_in(&self.values, lanes);
        // The float literals of an int column compare with each row widened
        // to `f64`. A column without them skips the widening of its rows.
        if !self.widened.is_empty() {
            hits |= lanes_in(&self.widened, T::convert_lanes::<f64>(lanes));
        }
        hits
    }

    fn one(&self, v: T) -> bool {
        // The guard skips the widening, as in `lanes`.
        contains(&self.values, v)
            || (!self.widened.is_empty() && contains(&self.widened, v.convert()))
    }
}

/// Builds the loop that marks the rows of number field `f` whose value is in
/// `set`.
fn number_loop<T: Number>(f: usize, set: NumberSet<T>) -> ChunkLoop {
    if set.is_empty() {
        return constant_loop(false);
    }
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            pack(values_of::<T>(cols, f), start, rows, out, &set);
        },
    )
}

/// Builds the loop that marks the rows of boolean field `f` that hold one of
/// the booleans in `kept`.
fn flags_loop(f: usize, kept: &[&Literal]) -> ChunkLoop {
    let has = |b: bool| {
        kept.iter()
            .any(|l| matches!(l, Literal::Bool(x) if *x == b))
    };
    match (has(true), has(false)) {
        // Both booleans are in the list, so every row with a value is in it.
        (true, true) => constant_loop(true),
        (false, false) => constant_loop(false),
        // One boolean is in the list. `value` is true when that boolean is
        // `True`, and the loop marks the rows that hold it.
        (value, _) => booleans_equal(f, value),
    }
}

/// The texts of a membership step in both encodings, each list sorted, so
/// that a row is looked up in the list of its column's encoding.
pub(super) struct TextSet {
    utf8: Vec<Box<[u8]>>,
    utf32: Vec<Box<[u32]>>,
}

impl TextSet {
    /// Builds the set of the text literals in `kept`.
    pub(super) fn of(kept: &[&Literal]) -> Self {
        let texts: Vec<_> = kept
            .iter()
            .map(|l| match l {
                Literal::Text(t) => t,
                _ => unreachable!("the caller keeps only texts for a text column"),
            })
            .collect();
        let mut utf8: Vec<Box<[u8]>> = texts.iter().map(|t| Box::from(t.utf8())).collect();
        let mut utf32: Vec<Box<[u32]>> = texts.iter().map(|t| Box::from(t.utf32())).collect();
        utf8.sort();
        utf8.dedup();
        utf32.sort();
        utf32.dedup();
        Self { utf8, utf32 }
    }
}

impl<'a, T: Texts<'a, Unit: Encoded + 'a>> Test<TextRows<'a, T>> for TextSet {
    // As for `AgainstText` in `text.rs`, `pack` reads every row through `one`.
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    fn one(&self, row: &'a [T::Unit]) -> bool {
        T::Unit::pick(&self.utf8, &self.utf32)
            .binary_search_by(|t| (**t).cmp(row))
            .is_ok()
    }
}

/// Builds the loop that marks the rows of text field `f` whose text is in
/// `set`.
fn text_loop(f: usize, set: TextSet) -> ChunkLoop {
    // Both lists hold the same texts, so one of them tells whether the set
    // is empty.
    if set.utf8.is_empty() {
        return constant_loop(false);
    }
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            // Picks the layout of this batch's column, and runs the `pack`
            // that was compiled for that layout.
            with_texts!(texts_of(cols, f), |t| pack(
                TextRows::new(t),
                start,
                rows,
                out,
                &set
            ));
        },
    )
}
