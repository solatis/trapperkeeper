//! The loops that compare texts, one per pair of encodings. Each reads its
//! text columns through `TextRows` or `TextRowPair`, so `pack` runs it like
//! any other comparison.
//!
//! A text literal has a form in each encoding, so a loop compares it with a
//! column's units as they are. Two columns of the same encoding compare their
//! units too. A UTF-8 column and a UTF-32 column compare code points, which
//! decodes the UTF-8 side of each row.

use std::sync::Arc;

use crate::column::{
    Column, Kind, Present, Test, Text, TextRowPair, TextRows, Texts, Unit, pack, with_texts,
};
use crate::plan::TextLiteral;

use super::bind::ChunkLoop;

/// A unit of text, a byte of UTF-8 or a unit of UTF-32. A text literal is
/// held in both, so that a loop compares it in the column's encoding.
// `Ord` lets the text set of a membership step sort its texts and search
// them.
pub(super) trait Encoded: Unit + Ord {
    /// Returns `literal` in this encoding.
    fn of(literal: &TextLiteral) -> &[Self];

    /// Picks the list in this encoding from a set of texts that a membership
    /// step holds in both encodings.
    fn pick<'s>(utf8: &'s [Box<[u8]>], utf32: &'s [Box<[u32]>]) -> &'s [Box<[Self]>];

    /// Returns the text `units` tagged with its encoding.
    fn form(units: &[Self]) -> Form<'_>;
}

/// The units of a text, tagged with their encoding.
#[derive(Clone, Copy)]
pub(super) enum Form<'a> {
    Utf8(&'a [u8]),
    Utf32(&'a [u32]),
}

impl Encoded for u8 {
    fn of(literal: &TextLiteral) -> &[Self] {
        literal.utf8()
    }

    fn form(units: &[Self]) -> Form<'_> {
        Form::Utf8(units)
    }

    fn pick<'s>(utf8: &'s [Box<[u8]>], _: &'s [Box<[u32]>]) -> &'s [Box<[Self]>] {
        utf8
    }
}

impl Encoded for u32 {
    fn of(literal: &TextLiteral) -> &[Self] {
        literal.utf32()
    }

    fn form(units: &[Self]) -> Form<'_> {
        Form::Utf32(units)
    }

    fn pick<'s>(_: &'s [Box<[u8]>], utf32: &'s [Box<[u32]>]) -> &'s [Box<[Self]>] {
        utf32
    }
}

/// What `pack` runs to compute `x == literal` or `x != literal` on a text
/// column.
struct AgainstText {
    literal: Arc<TextLiteral>,
    equal: bool,
}

impl<'a, T: Texts<'a, Unit: Encoded + 'a>> Test<TextRows<'a, T>> for AgainstText {
    // A text source has no lanes, because `TextRows` tells `pack` that its
    // rows do not lie next to each other, so `pack` reads every row through
    // `one`.
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    fn one(&self, row: &'a [T::Unit]) -> bool {
        // `of` gives the literal in the column's encoding (ADR-0041). A row
        // without a value compares whatever units it holds, and
        // `StepResult::compared` makes it unknown.
        (row == T::Unit::of(&self.literal)) == self.equal
    }
}

/// Builds the loop that marks the rows where text field `f` equals
/// `literal`, or differs from it when `equal` is false.
pub(super) fn equals_literal(f: usize, literal: Arc<TextLiteral>, equal: bool) -> ChunkLoop {
    let test = AgainstText { literal, equal };
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            // Bind only knows that the column holds texts. Each batch may hold
            // them in another layout, so the loop picks the layout each time it
            // runs, which is once per chunk.
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

/// What `pack` runs to compute `x == y` or `x != y` on two text columns.
struct BetweenTexts {
    equal: bool,
}

impl<'a, L, R> Test<TextRowPair<'a, L, R>> for BetweenTexts
where
    L: Texts<'a, Unit: Encoded + 'a>,
    R: Texts<'a, Unit: Encoded + 'a>,
{
    // As for `AgainstText`, `pack` reads every row through `one`.
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    fn one(&self, (left, right): (&'a [L::Unit], &'a [R::Unit])) -> bool {
        same_text(L::Unit::form(left), R::Unit::form(right)) == self.equal
    }
}

/// Builds the loop that marks the rows where text fields `l` and `r` are
/// equal, or differ when `equal` is false.
pub(super) fn fields_equal(l: usize, r: usize, equal: bool) -> ChunkLoop {
    let test = BetweenTexts { equal };
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            // The two layouts are known only per batch, so the loop is
            // compiled for each pair of layouts and picks its pair per chunk.
            with_texts!(texts_of(cols, l), |x| with_texts!(texts_of(cols, r), |y| {
                pack(TextRowPair::new(x, y), start, rows, out, &test);
            }));
        },
    )
}

/// Returns whether two texts hold the same characters, in whatever
/// encodings they come.
pub(super) fn same_text(left: Form<'_>, right: Form<'_>) -> bool {
    match (left, right) {
        // Within one encoding, two texts are equal when their units are.
        (Form::Utf8(a), Form::Utf8(b)) => a == b,
        (Form::Utf32(a), Form::Utf32(b)) => a == b,
        // A UTF-8 text and a UTF-32 text are equal when their code points
        // are (ADR-0041).
        (Form::Utf8(bytes), Form::Utf32(units)) | (Form::Utf32(units), Form::Utf8(bytes)) => {
            same_code_points(bytes, units)
        }
    }
}

/// Returns whether the UTF-8 text `bytes` holds the code points `units`, in
/// order.
fn same_code_points(bytes: &[u8], units: &[u32]) -> bool {
    // Every text layout whose units are bytes holds UTF-8 in every row, also
    // in a row without a value (`Texts`), so the decode cannot fail. The
    // checked decode keeps the loop free of `unsafe`, which
    // `from_utf8_unchecked` would need.
    let text = str::from_utf8(bytes).expect("a text layout of bytes holds UTF-8");
    // A UTF-32 unit that is no code point, such as a lone surrogate, equals
    // no character.
    text.chars().map(u32::from).eq(units.iter().copied())
}

/// Returns the layout of text field `f`.
pub(super) fn texts_of<'a>(cols: &[Option<Column<'a>>], f: usize) -> Text<'a> {
    match cols[f].as_ref().map(Column::kind) {
        Some(Kind::Text(t, _)) => *t,
        // Bind made this loop for a text column, and `run` refuses a batch
        // whose column has another type. A field without a column gets no
        // loop, because bind makes its step a constant.
        _ => unreachable!("run checked the type of the column"),
    }
}

/// What `pack` runs to compute `element == needle` for each element of a
/// text column and one needle text, which a list search takes from the needle
/// field of a record.
pub(super) struct EqualsText<'n> {
    pub(super) needle: Form<'n>,
}

impl<'a, T: Texts<'a, Unit: Encoded + 'a>> Test<TextRows<'a, T>> for EqualsText<'_> {
    // As for `AgainstText`, `pack` reads every row through `one`.
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    fn one(&self, element: &'a [T::Unit]) -> bool {
        same_text(self.needle, T::Unit::form(element))
    }
}
