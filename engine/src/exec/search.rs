//! The two meanings of `x in y` with a field `y`: substring search, when `y`
//! holds a text, and list search, when `y` holds a list. Bind picks one from
//! the type of `y` in each schema, so one rule may search a text on one
//! schema and a list on another.

use std::cell::RefCell;
use std::ops::Range;

use regex::bytes::Regex;

use super::bind::{
    BindError, BoundStep, ChunkLoop, FieldType, SearchLoop, invert, write_booleans_of,
};
use super::member::{NumberSet, TextSet};
use super::number::EqualsNumber;
use super::outcome::Fault;
use super::text::{Encoded, EqualsText, Form, texts_of};
use crate::column::{
    AnyOffsets, AnySized, Booleans, CHUNK_ROWS, CHUNK_WORDS, Column, Kind, List, Lists, Number,
    NumberType, Present, Scalar, Shape, Test, TextRowPair, TextRows, Texts, pack, present_all,
    with_float, with_int, with_number, with_texts,
};
use crate::plan::{Constant, Literal, Operand, TextLiteral};

/// Builds the step of `needle in y`, where `y` is field `haystack` and
/// `types` holds the column type of each field. The step searches the text or
/// the elements of `y`, and faults where `y` holds a value that cannot be
/// searched for the needle. Fails for a needle that is a list field, because
/// list equality does not exist.
pub(super) fn bind_search(
    needle: &Operand,
    haystack: usize,
    types: &[FieldType],
) -> Result<BoundStep, BindError> {
    // The cases are the types of the needle and of `y`, and the first that
    // applies returns. The fields without a column come first, because a
    // search of a null, or for a null, is unknown whatever the other side is
    // (ADR-0005). A list field as the needle comes next, so that bind refuses
    // it whatever `y` holds when `y` has a column.
    let needle_type = match needle {
        Operand::Field(n) => Some(types[*n]),
        Operand::Literal(_) => None,
    };
    // The search reads the haystack and a needle field, and a row where one
    // of them has no value stays unknown.
    let fields = match needle {
        Operand::Field(n) => vec![*n, haystack],
        Operand::Literal(_) => vec![haystack],
    };
    Ok(match (needle, needle_type, types[haystack]) {
        (_, _, FieldType::Absent) | (_, Some(FieldType::Absent), _) => {
            BoundStep::Constant(Constant::Unknown)
        }
        (Operand::Field(n), Some(FieldType::List(_)), _) => {
            return Err(BindError::Unsupported {
                left: *n,
                right: haystack,
            });
        }
        (Operand::Literal(Literal::Text(text)), _, FieldType::Text) => BoundStep::Loop {
            fields,
            chunk: substring_literal(haystack, text),
        },
        (Operand::Field(n), Some(FieldType::Text), FieldType::Text) => BoundStep::Loop {
            fields,
            chunk: substring_field(*n, haystack),
        },
        (_, _, FieldType::List(shape)) => BoundStep::Search {
            fields,
            chunk: list_search(haystack, Needle::of(needle, needle_type, shape)),
        },
        // A text cannot be searched for a needle that is no text, and a
        // number or a boolean cannot be searched at all, as Python raises a
        // type error for `5 in "abc"` and for `5 in 3`.
        (_, _, FieldType::Text | FieldType::Number(_) | FieldType::Bool) => BoundStep::Faulted {
            fault: Fault::Incompatible,
            fields,
        },
    })
}

/// Builds the step of `None in y`, where `y` is field `haystack` and its
/// column has type `t`: true where the list `y` holds a null element, and
/// false where it holds none or is empty.
pub(super) fn bind_holds_null(haystack: usize, t: FieldType) -> BoundStep {
    let fields = vec![haystack];
    match t {
        FieldType::Absent => BoundStep::Constant(Constant::Unknown),
        FieldType::List(_) => BoundStep::Search {
            fields,
            chunk: null_search(haystack),
        },
        // `None` cannot be searched for in a text, as Python raises a type
        // error for `None in "abc"`, and a number or a boolean cannot be
        // searched at all.
        FieldType::Text | FieldType::Number(_) | FieldType::Bool => BoundStep::Faulted {
            fault: Fault::Incompatible,
            fields,
        },
    }
}

/// What `pack` runs to find the text literal `needle` in each row of a text
/// column.
struct ContainsText {
    // The escaped literal, which `regex` compiles into a substring search
    // over bytes, so a UTF-8 row needs no check that its bytes are UTF-8.
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

/// What `pack` runs to find the text of a needle field in the text of a
/// haystack field, row by row.
struct ContainsField {
    // A UTF-8 text decodes into these units when the other side is UTF-32.
    // The buffer lives across the rows of a chunk, so a row allocates only
    // when its text is longer than every text before it.
    units: RefCell<Vec<u32>>,
}

impl<'a, N, H> Test<TextRowPair<'a, N, H>> for ContainsField
where
    N: Texts<'a, Unit: Encoded + 'a>,
    H: Texts<'a, Unit: Encoded + 'a>,
{
    // As for `AgainstText` in `text.rs`, `pack` reads every row through `one`.
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    fn one(&self, (needle, haystack): (&'a [N::Unit], &'a [H::Unit])) -> bool {
        let mut units = self.units.borrow_mut();
        match (N::Unit::form(needle), H::Unit::form(haystack)) {
            // The needle changes from row to row, and a pattern built per row
            // would cost more than one `str::contains`, so this case uses no
            // `regex`.
            (Form::Utf8(n), Form::Utf8(h)) => utf8(h).contains(utf8(n)),
            (Form::Utf32(n), Form::Utf32(h)) => contains_units(h, n),
            (Form::Utf8(n), Form::Utf32(h)) => contains_units(h, decoded(n, &mut units)),
            (Form::Utf32(n), Form::Utf8(h)) => contains_units(decoded(h, &mut units), n),
        }
    }
}

/// Returns the UTF-8 text `bytes` as a `str`.
fn utf8(bytes: &[u8]) -> &str {
    // Every text layout whose units are bytes holds UTF-8 in every row, so
    // the check passes. It keeps the loop free of `unsafe`, which
    // `from_utf8_unchecked` would need.
    str::from_utf8(bytes).expect("a text layout of bytes holds UTF-8")
}

/// Decodes the UTF-8 text `bytes` into `units`, one unit per character, and
/// returns them.
fn decoded<'u>(bytes: &[u8], units: &'u mut Vec<u32>) -> &'u [u32] {
    units.clear();
    units.extend(utf8(bytes).chars().map(u32::from));
    units
}

/// Builds the loop that marks the rows where the text of field `n` occurs in
/// the text of field `h`.
fn substring_field(n: usize, h: usize) -> ChunkLoop {
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            // `ContainsField` holds a `RefCell`, which is not `Sync`, so each
            // call builds its own test and the boxed loop stays `Send + Sync`.
            let test = ContainsField {
                units: RefCell::new(Vec::new()),
            };
            // The two layouts are known only per batch, so the loop is
            // compiled for each pair of layouts and picks its pair per chunk.
            with_texts!(texts_of(cols, n), |x| with_texts!(texts_of(cols, h), |y| {
                pack(TextRowPair::new(x, y), start, rows, out, &test);
            }));
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
    // A field, whose value on each record is the needle of that record.
    Field(usize),
}

impl Needle {
    /// Decides what a list search compares each element with, from the
    /// needle, its column type `needle_type` when it is a field, and the shape
    /// of the list's records. Returns `Never` when no element can equal the
    /// needle.
    fn of(needle: &Operand, needle_type: Option<FieldType>, shape: Shape) -> Self {
        // A record of depth 1 holds scalars. A deeper record holds lists, and
        // no scalar equals a list.
        let scalar = (shape.depth == 1).then_some(shape.scalar);
        // A boolean never equals a number, for a literal and for a field, so
        // `1 in [True]` is false, while Python says it is true (ADR-0041).
        let can_equal = match (needle, needle_type) {
            (Operand::Literal(l), _) => matches!(
                (scalar, l),
                (
                    Some(Scalar::Int | Scalar::Float),
                    Literal::Int(_) | Literal::Float(_)
                ) | (Some(Scalar::Bool), Literal::Bool(_))
                    | (Some(Scalar::Text), Literal::Text(_))
            ),
            (Operand::Field(_), t) => matches!(
                (scalar, t),
                (
                    Some(Scalar::Int | Scalar::Float),
                    Some(FieldType::Number(_))
                ) | (Some(Scalar::Bool), Some(FieldType::Bool))
                    | (Some(Scalar::Text), Some(FieldType::Text))
            ),
        };
        match needle {
            _ if !can_equal => Self::Never,
            Operand::Literal(l) => Self::Literal(l.clone()),
            Operand::Field(n) => Self::Field(*n),
        }
    }
}

/// The test of one element column: it marks the rows in `start .. start +
/// rows` whose element equals the needle.
type ElementTest<'c> = Box<dyn Fn(usize, usize, &mut Present) + 'c>;

/// Returns the test of the element column `elements` against `needle`. A
/// needle field takes its value from row `row` of `cols`. On a row where the
/// needle has no value, the test compares whatever its memory holds, and
/// `StepResult::searched` makes the row unknown.
fn element_test<'c>(
    needle: &Needle,
    cols: &[Option<Column<'c>>],
    row: usize,
    elements: &'c Column<'c>,
) -> ElementTest<'c> {
    match needle {
        Needle::Never => never(),
        Needle::Literal(literal) => literal_test(literal, elements),
        // Bind made a field needle for a field that has a column.
        Needle::Field(n) => {
            field_test(cols[*n].as_ref().expect("bind saw a column"), row, elements)
        }
    }
}

/// Returns the test that marks no element.
fn never<'c>() -> ElementTest<'c> {
    Box::new(|_, _, out: &mut Present| out.fill(0))
}

/// Returns the test of the element column `elements` against `literal`.
fn literal_test<'c>(literal: &Literal, elements: &'c Column<'c>) -> ElementTest<'c> {
    // The sets of `member.rs` convert the literal to the element type as
    // `==` does, so a list search and a membership agree on every pair. The
    // sets take their values as a list of literals, which here holds one.
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
        (Kind::Booleans(..), Literal::Bool(b)) => booleans_test(elements, *b),
        (Kind::Text(text, _), Literal::Text(_)) => {
            let (text, set) = (*text, TextSet::of(&kept));
            Box::new(move |start, rows, out: &mut Present| {
                with_texts!(text, |t| pack(TextRows::new(t), start, rows, out, &set));
            })
        }
        // `Needle::of` keeps a literal only for elements of its kind.
        _ => never(),
    }
}

/// Returns the test that marks the number elements found in `set`.
fn number_test<'c, T: Number>(elements: &'c Column<'c>, set: NumberSet<T>) -> ElementTest<'c> {
    // The set is empty when `T` cannot hold the literal, as 300 for a `u8`
    // column.
    if set.is_empty() {
        return never();
    }
    let values = T::values(elements.kind()).expect("the set was built for the element type");
    Box::new(move |start, rows, out: &mut Present| pack(values, start, rows, out, &set))
}

/// Returns the test that marks the boolean elements equal to `value`.
fn booleans_test<'c>(elements: &'c Column<'c>, value: bool) -> ElementTest<'c> {
    Box::new(move |start, rows, out: &mut Present| {
        write_booleans_of(elements, start, rows, out);
        if !value {
            invert(out, rows);
        }
    })
}

/// Returns the test of the element column `elements` against the value of
/// row `row` of the needle column `needle`.
fn field_test<'c>(needle: &Column<'c>, row: usize, elements: &'c Column<'c>) -> ElementTest<'c> {
    match (needle.kind(), elements.kind()) {
        (Kind::Ints(..) | Kind::Floats(..), Kind::Ints(..) | Kind::Floats(..)) => {
            // `with_number!` expands into one arm per number type, so the
            // comparison is compiled for the needle's type `L` and the
            // elements' type `R`, as `x == y` on two number fields is.
            with_number!(number_type(needle), |L| needle_number::<L>(
                needle, row, elements
            ))
        }
        (Kind::Booleans(b, _), Kind::Booleans(..)) => {
            // A byte that is not zero is true, as `write_booleans_of` reads it.
            let value = match b {
                Booleans::Bits(bits) => bits.get(row),
                Booleans::Bytes(bytes) => bytes.get(row) != 0,
            };
            booleans_test(elements, value)
        }
        (Kind::Text(n, _), Kind::Text(e, _)) => {
            let (n, e) = (*n, *e);
            Box::new(move |start, rows, out: &mut Present| {
                // The needle keeps its own encoding, and `EqualsText` compares
                // it with each element in any pair of encodings.
                with_texts!(n, |t| {
                    let test = EqualsText {
                        needle: Encoded::form(Texts::get(&t, row)),
                    };
                    with_texts!(e, |u| pack(TextRows::new(u), start, rows, out, &test));
                });
            })
        }
        // `Needle::of` keeps a field only for elements of its kind.
        _ => never(),
    }
}

/// Returns the test of the number elements `elements` against row `row` of
/// the needle column `needle`, which holds numbers of type `L`.
fn needle_number<'c, L: Number>(
    needle: &Column<'c>,
    row: usize,
    elements: &'c Column<'c>,
) -> ElementTest<'c> {
    // A row without a value reads whatever the memory holds, as the doc
    // comment of `element_test` says.
    let value = L::values(needle.kind()).expect("a number column").get(row);
    with_number!(number_type(elements), |R| pair_test::<L, R>(
        value, elements
    ))
}

/// Returns the test that marks the elements of type `R` that equal `value`.
fn pair_test<'c, L: Number, R: Number>(value: L, elements: &'c Column<'c>) -> ElementTest<'c> {
    let values = R::values(elements.kind()).expect("a number column");
    let test = EqualsNumber::<L, R>::new(value);
    Box::new(move |start, rows, out: &mut Present| pack(values, start, rows, out, &test))
}

/// Returns the number type of a number column.
fn number_type(c: &Column<'_>) -> NumberType {
    match c.kind() {
        Kind::Ints(v, _) => v.number_type(),
        Kind::Floats(v, _) => v.number_type(),
        _ => unreachable!("the caller checked that the column holds numbers"),
    }
}

/// Searches the rows `range` of the element column `elements` with `test`,
/// and returns whether an element matched and whether an element is a null.
/// It reads the range in parts of up to `CHUNK_ROWS` elements and stops after
/// the part with the first match. The null answer then covers only the
/// elements up to the end of that part, which does not matter, because a
/// match decides the search on its own.
fn scan(elements: &Column<'_>, range: Range<usize>, test: &ElementTest<'_>) -> (bool, bool) {
    // A list may hold more elements than a chunk has rows.
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
            for_each_record(
                cols,
                lists_of(cols, haystack),
                start,
                rows,
                &needle,
                &mut mark,
            );
        },
    )
}

/// Builds the loop of `None in y`, where `y` is the list field `haystack`. It
/// marks the records that hold a null element and leaves none undecided.
fn null_search(haystack: usize) -> SearchLoop {
    Box::new(
        move |cols: &[Option<Column<'_>>],
              start: usize,
              rows: usize,
              matched: &mut Present,
              undecided: &mut Present| {
            matched.fill(0);
            undecided.fill(0);
            // `Needle::Never` matches no element, so `scan` reports only
            // whether a record has a null element, which marks the record.
            let mut mark = |row: usize, (_, null): (bool, bool)| {
                matched[row / 64] |= u64::from(null) << (row % 64);
            };
            for_each_record(
                cols,
                lists_of(cols, haystack),
                start,
                rows,
                &Needle::Never,
                &mut mark,
            );
        },
    )
}

/// Scans the elements of each record in `start .. start + rows` of `list`
/// for `needle`, and passes the record's row in the chunk and the result of
/// `scan` to `mark`.
fn for_each_record<'c>(
    cols: &[Option<Column<'c>>],
    list: List<'c>,
    start: usize,
    rows: usize,
    needle: &Needle,
    mark: &mut impl FnMut(usize, (bool, bool)),
) {
    // `storage` and `elements` both give the element column of a layout.
    match list {
        List::Offsets(AnyOffsets::I32(o)) => {
            shared(cols, o, o.storage(), start, rows, needle, mark);
        }
        List::Offsets(AnyOffsets::I64(o)) => {
            shared(cols, o, o.storage(), start, rows, needle, mark);
        }
        List::Offsets(AnyOffsets::U32(o)) => {
            shared(cols, o, o.storage(), start, rows, needle, mark);
        }
        List::Offsets(AnyOffsets::U64(o)) => {
            shared(cols, o, o.storage(), start, rows, needle, mark);
        }
        List::Fixed(f) => shared(cols, f, f.storage(), start, rows, needle, mark),
        List::Sized(AnySized::I32(s)) => shared(cols, s, s.elements(), start, rows, needle, mark),
        List::Sized(AnySized::I64(s)) => shared(cols, s, s.elements(), start, rows, needle, mark),
        List::Sized(AnySized::U32(s)) => shared(cols, s, s.elements(), start, rows, needle, mark),
        List::Sized(AnySized::U64(s)) => shared(cols, s, s.elements(), start, rows, needle, mark),
        // Each record is a column of its own, which gets a test of its own.
        List::PerRecord(p) => {
            for row in 0..rows {
                let record = p.get(start + row);
                let test = element_test(needle, cols, start + row, record);
                mark(row, scan(record, 0..record.rows(), &test));
            }
        }
    }
}

/// Scans the records of a layout whose records are ranges of one element
/// column, `elements`.
fn shared<'a, L: Lists<'a, Record = Range<usize>>>(
    cols: &[Option<Column<'a>>],
    layout: L,
    elements: &'a Column<'a>,
    start: usize,
    rows: usize,
    needle: &Needle,
    mark: &mut impl FnMut(usize, (bool, bool)),
) {
    // A literal needle is the same on every record, so the chunk builds its
    // test once. A needle field gives each record a test of its own value.
    let once = match needle {
        Needle::Field(_) => None,
        _ => Some(element_test(needle, cols, start, elements)),
    };
    for row in 0..rows {
        let range = layout.get(start + row);
        if let Some(test) = &once {
            mark(row, scan(elements, range, test));
        } else {
            let test = element_test(needle, cols, start + row, elements);
            mark(row, scan(elements, range, &test));
        }
    }
}

/// Returns the layout of list field `f`.
fn lists_of<'a>(cols: &[Option<Column<'a>>], f: usize) -> List<'a> {
    match cols[f].as_ref().map(Column::kind) {
        Some(Kind::List(l, _)) => *l,
        // Bind made this loop for a list column, and `run` refuses a batch
        // whose column has another type. A field without a column gets no
        // loop, because bind makes its step a constant.
        _ => unreachable!("run checked the type of the column"),
    }
}
