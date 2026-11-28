//! The two meanings of `x in y` with a field `y`: substring search, when `y`
//! holds a text, and list search, when `y` holds a list. Bind picks one from
//! the type of `y` in each schema, so one rule may search a text on one
//! schema and a list on another.

use std::ops::Range;

use regex::bytes::Regex;

use super::bind::{
    BindError, BoundStep, ChunkLoop, FieldType, SearchLoop, invert, write_booleans_of,
};
use super::member::{NumberSet, TextSet};
use super::outcome::Fault;
use super::text::{Encoded, Form, texts_of};
use crate::column::{
    AnyOffsets, AnySized, CHUNK_ROWS, CHUNK_WORDS, Column, Kind, List, Lists, Number, Present,
    Scalar, Shape, Test, TextRows, Texts, pack, present_all, with_float, with_int, with_texts,
};
use crate::plan::{Constant, Literal, Operand, TextLiteral};

/// Builds the step of `needle in y`, where `y` is field `haystack` and
/// `types` holds the column type of each field. The step searches the text or
/// the elements of `y`, and faults where `y` holds a value that cannot be
/// searched for the needle. Fails for a needle field, which this version
/// cannot search for yet.
pub(super) fn bind_search(
    needle: &Operand,
    haystack: usize,
    types: &[FieldType],
) -> Result<BoundStep, BindError> {
    // The cases are the types of `y`, and the first that applies returns.
    // The missing field comes first, because a search of a null is unknown
    // whatever the needle is (ADR-0005).
    let fields = vec![haystack];
    Ok(match (needle, types[haystack]) {
        (_, FieldType::Absent) => BoundStep::Constant(Constant::Unknown),
        (Operand::Literal(Literal::Text(text)), FieldType::Text) => BoundStep::Loop {
            fields,
            chunk: substring_literal(haystack, text),
        },
        (Operand::Field(n), _) => {
            return Err(BindError::Unsupported {
                left: *n,
                right: haystack,
            });
        }
        (Operand::Literal(literal), FieldType::List(shape)) => BoundStep::Search {
            fields,
            chunk: list_search(haystack, Needle::of(literal, shape)),
        },
        // A text cannot be searched for a needle that is no text, and a
        // number or a boolean cannot be searched at all, as Python raises a
        // type error for `5 in "abc"` and for `5 in 3`.
        (Operand::Literal(_), FieldType::Text | FieldType::Number(_) | FieldType::Bool) => {
            BoundStep::Faulted {
                fault: Fault::Incompatible,
                fields,
            }
        }
    })
}

/// What `pack` runs to find the text literal `needle` in each row of a text
/// column.
struct ContainsText {
    // The escaped literal, which `regex` compiles into a substring search
    // over bytes, so a UTF-8 row needs no check before the search.
    utf8: Regex,
    utf32: Box<[u32]>,
}

impl<'a, T: Texts<'a, Unit: Encoded + 'a>> Test<TextRows<'a, T>> for ContainsText {
    // As for `AgainstText` in `text.rs`, `pack` reads every row through `one`.
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    fn one(&self, row: &'a [T::Unit]) -> bool {
        match T::Unit::form(row) {
            Form::Utf8(bytes) => self.utf8.is_match(bytes),
            // The `regex` crate matches UTF-8 only, so a UTF-32 row is
            // searched unit by unit (ADR-0041).
            Form::Utf32(units) => contains_units(units, &self.utf32),
        }
    }
}

/// Returns whether `needle` occurs in `haystack`, unit for unit. The empty
/// needle occurs in every text, as `"" in s` is true in Python.
fn contains_units(haystack: &[u32], needle: &[u32]) -> bool {
    // `windows` panics on a width of 0, so the empty needle returns first.
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// Builds the loop that marks the rows of text field `f` that contain the
/// text literal `needle`.
fn substring_literal(f: usize, needle: &TextLiteral) -> ChunkLoop {
    // An escaped literal is a valid pattern, and a rule's text is far below
    // the size limit of a compiled pattern.
    let utf8 = Regex::new(&regex::escape(needle.utf8_str())).expect("an escaped literal compiles");
    let test = ContainsText {
        utf8,
        utf32: needle.utf32().into(),
    };
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            // Picks the layout of this batch's column, and runs the `pack`
            // that was compiled for that layout.
            with_texts!(texts_of(cols, f), |t| pack(
                TextRows::new(t),
                start,
                rows,
                out,
                &test
            ));
        },
    )
}

/// What a list search compares each element with.
#[derive(Debug, Clone)]
enum Needle {
    // No element can equal the needle, such as a text needle among ints, so
    // the search only looks for null elements.
    Never,
    Literal(Literal),
}

impl Needle {
    /// Returns what a search for `literal` compares with the elements of a
    /// list whose records have shape `shape`.
    fn of(literal: &Literal, shape: Shape) -> Self {
        // A record of depth 1 holds scalars. A deeper record holds lists, and
        // no literal equals a list.
        let scalar = (shape.depth == 1).then_some(shape.scalar);
        let can_equal = matches!(
            (scalar, literal),
            (
                Some(Scalar::Int | Scalar::Float),
                Literal::Int(_) | Literal::Float(_)
            ) | (Some(Scalar::Bool), Literal::Bool(_))
                | (Some(Scalar::Text), Literal::Text(_))
        );
        if can_equal {
            Self::Literal(literal.clone())
        } else {
            Self::Never
        }
    }
}

/// The test of one element column: it marks the rows in `start .. start +
/// rows` whose element equals the needle.
type ElementTest<'c> = Box<dyn Fn(usize, usize, &mut Present) + 'c>;

/// Returns the test of the element column `elements` against `needle`.
fn element_test<'c>(needle: &Needle, elements: &'c Column<'c>) -> ElementTest<'c> {
    let never: ElementTest<'c> = Box::new(|_, _, out: &mut Present| out.fill(0));
    let Needle::Literal(literal) = needle else {
        return never;
    };
    // The sets of `member.rs` convert the literal to the element type as
    // `==` does, so a list search and a membership agree on every pair.
    let kept = [literal];
    match (elements.kind(), literal) {
        (Kind::Ints(v, _), Literal::Int(_) | Literal::Float(_)) => {
            // `with_int!` and `with_float!` expand into one arm per number
            // type, so the set is built for the type of the elements.
            with_int!(v.number_type(), |T| number_test(
                elements,
                NumberSet::<T>::for_int(&kept)
            ))
        }
        (Kind::Floats(v, _), Literal::Int(_) | Literal::Float(_)) => {
            with_float!(v.number_type(), |T| number_test(
                elements,
                NumberSet::<T>::for_float(&kept)
            ))
        }
        (Kind::Booleans(..), Literal::Bool(b)) => {
            let value = *b;
            Box::new(move |start, rows, out: &mut Present| {
                write_booleans_of(elements, start, rows, out);
                if !value {
                    invert(out, rows);
                }
            })
        }
        (Kind::Text(text, _), Literal::Text(_)) => {
            let (text, set) = (*text, TextSet::of(&kept));
            Box::new(move |start, rows, out: &mut Present| {
                with_texts!(text, |t| pack(TextRows::new(t), start, rows, out, &set));
            })
        }
        // `Needle::of` keeps a literal only for elements of its kind.
        _ => never,
    }
}

/// Returns the test that marks the number elements found in `set`.
fn number_test<'c, T: Number>(elements: &'c Column<'c>, set: NumberSet<T>) -> ElementTest<'c> {
    if set.is_empty() {
        return Box::new(|_, _, out: &mut Present| out.fill(0));
    }
    let values = T::values(elements.kind()).expect("the set was built for the element type");
    Box::new(move |start, rows, out: &mut Present| pack(values, start, rows, out, &set))
}

/// Searches the rows `range` of the element column `elements` with `test`,
/// and returns whether an element matched and whether an element is a null.
fn scan(elements: &Column<'_>, range: Range<usize>, test: &ElementTest<'_>) -> (bool, bool) {
    // A list may hold more elements than a chunk has rows, so the range is
    // read in parts of up to `CHUNK_ROWS`, and the scan stops at the first
    // match.
    let (mut hit, mut null) = (false, false);
    let mut at = range.start;
    while at < range.end && !hit {
        let rows = (range.end - at).min(CHUNK_ROWS);
        let (mut present, mut all, mut equal) =
            ([0; CHUNK_WORDS], [0; CHUNK_WORDS], [0; CHUNK_WORDS]);
        elements.present(at, rows, &mut present);
        present_all(rows, &mut all);
        test(at, rows, &mut equal);
        null |= present != all;
        // An element without a value matches nothing, whatever its memory
        // holds.
        hit |= equal.iter().zip(&present).any(|(e, p)| e & p != 0);
        at += rows;
    }
    (hit, null)
}

/// Builds the loop of `needle in y`, where `y` is the list field `haystack`.
/// It writes, per record, whether an element matched and whether the record
/// is undecided because of a null element.
fn list_search(haystack: usize, needle: Needle) -> SearchLoop {
    Box::new(
        move |cols: &[Option<Column<'_>>],
              start: usize,
              rows: usize,
              matched: &mut Present,
              undecided: &mut Present| {
            matched.fill(0);
            undecided.fill(0);
            // A record is matched when an element equals the needle, and
            // undecided when none does and an element is a null, as `any`
            // over the equalities gives (ADR-0005).
            let mut mark = |row: usize, (hit, null): (bool, bool)| {
                matched[row / 64] |= u64::from(hit) << (row % 64);
                undecided[row / 64] |= u64::from(!hit && null) << (row % 64);
            };
            for_each_record(lists_of(cols, haystack), start, rows, &needle, &mut mark);
        },
    )
}

/// Scans the elements of each record in `start .. start + rows` of `list`
/// for `needle`, and passes the record's row in the chunk and the result of
/// `scan` to `mark`.
fn for_each_record(
    list: List<'_>,
    start: usize,
    rows: usize,
    needle: &Needle,
    mark: &mut impl FnMut(usize, (bool, bool)),
) {
    match list {
        List::Offsets(AnyOffsets::I32(o)) => shared(o, o.storage(), start, rows, needle, mark),
        List::Offsets(AnyOffsets::I64(o)) => shared(o, o.storage(), start, rows, needle, mark),
        List::Offsets(AnyOffsets::U32(o)) => shared(o, o.storage(), start, rows, needle, mark),
        List::Offsets(AnyOffsets::U64(o)) => shared(o, o.storage(), start, rows, needle, mark),
        List::Fixed(f) => shared(f, f.storage(), start, rows, needle, mark),
        List::Sized(AnySized::I32(s)) => shared(s, s.elements(), start, rows, needle, mark),
        List::Sized(AnySized::I64(s)) => shared(s, s.elements(), start, rows, needle, mark),
        List::Sized(AnySized::U32(s)) => shared(s, s.elements(), start, rows, needle, mark),
        List::Sized(AnySized::U64(s)) => shared(s, s.elements(), start, rows, needle, mark),
        // Each record is a column of its own, which gets a test of its own.
        List::PerRecord(p) => {
            for row in 0..rows {
                let record = p.get(start + row);
                let test = element_test(needle, record);
                mark(row, scan(record, 0..record.rows(), &test));
            }
        }
    }
}

/// Scans the records of a layout whose records are ranges of one element
/// column, `elements`, which gets one test for the whole chunk.
fn shared<'a, L: Lists<'a, Record = Range<usize>>>(
    layout: L,
    elements: &'a Column<'a>,
    start: usize,
    rows: usize,
    needle: &Needle,
    mark: &mut impl FnMut(usize, (bool, bool)),
) {
    let test = element_test(needle, elements);
    for row in 0..rows {
        mark(row, scan(elements, layout.get(start + row), &test));
    }
}

/// Returns the layout of list field `f`.
fn lists_of<'a>(cols: &[Option<Column<'a>>], f: usize) -> List<'a> {
    match cols[f].as_ref().map(Column::kind) {
        Some(Kind::List(l, _)) => *l,
        // Bind made this loop for a list column, and `run` refuses a batch
        // whose column has another type.
        _ => unreachable!("run checked the type of the column"),
    }
}
