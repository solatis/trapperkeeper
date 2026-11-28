//! The loops that compare texts. Each reads a text column through
//! `TextRows`, so `pack` runs it like any other comparison, and compares the
//! units of each row in the column's own encoding.

use std::sync::Arc;

use crate::column::{Column, Kind, Present, Test, Text, TextRows, Texts, Unit, pack, with_texts};
use crate::plan::TextLiteral;

use super::bind::ChunkLoop;

/// A unit of text in which a text literal has a form of its own: a byte of
/// UTF-8 or a unit of UTF-32.
trait Encoded: Unit {
    /// Returns `literal` in this encoding.
    fn of(literal: &TextLiteral) -> &[Self];
}

impl Encoded for u8 {
    fn of(literal: &TextLiteral) -> &[Self] {
        literal.utf8()
    }
}

impl Encoded for u32 {
    fn of(literal: &TextLiteral) -> &[Self] {
        literal.utf32()
    }
}

/// What `pack` runs to compute `x == literal` or `x != literal` on a text
/// column.
struct AgainstText {
    literal: Arc<TextLiteral>,
    equal: bool,
}

impl<'a, T: Texts<'a, Unit: Encoded + 'a>> Test<TextRows<'a, T>> for AgainstText {
    fn lanes(&self, (): ()) -> u64 {
        unreachable!("a text source is never contiguous")
    }

    // A row equals the literal when its units equal the literal's units in
    // the same encoding, so a UTF-32 layout compares code points and every
    // other layout compares bytes (ADR-0041).
    fn one(&self, row: &'a [T::Unit]) -> bool {
        (row == T::Unit::of(&self.literal)) == self.equal
    }
}

/// Builds the loop that marks the rows where text field `f` equals
/// `literal`, or differs from it when `equal` is false.
pub(super) fn equals_literal(f: usize, literal: Arc<TextLiteral>, equal: bool) -> ChunkLoop {
    let test = AgainstText { literal, equal };
    Box::new(
        move |cols: &[Option<Column<'_>>], start: usize, rows: usize, out: &mut Present| {
            // Bind knows that the column holds texts, and each batch may
            // hold them in another layout, so the loop picks the layout per
            // chunk.
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

/// Returns the layout of text field `f`.
fn texts_of<'a>(cols: &[Option<Column<'a>>], f: usize) -> Text<'a> {
    match cols[f].as_ref().map(Column::kind) {
        Some(Kind::Text(t, _)) => *t,
        // Bind made this loop for a text column, and `run` refuses a batch
        // whose column has another type.
        _ => unreachable!("run checked the type of the column"),
    }
}
