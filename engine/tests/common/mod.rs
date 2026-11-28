//! Strategies for the engine's tests: expression trees, and rules that hold
//! them. The module also holds the helpers that build and check the bitmap of
//! a chunk, and the texts and buffers of a text column in each layout.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::sample::{Index, select};
use proptest::test_runner::TestCaseError;
use trapperkeeper_engine::column::{
    AnyOffsets, CHUNK_ROWS, CHUNK_WORDS, Fixed, INLINE, Offsets, Piece, Present, Strided, Text,
    Units, Utf8, View, Views,
};
use trapperkeeper_engine::expr::{Expr, Segment, TypeName};
use trapperkeeper_engine::limits::{MAX_SCOPE_TAGS, MAX_TAG_LENGTH};
use trapperkeeper_engine::{Action, Rule};
use uuid::Uuid;

/// The builder of a node with one operand.
type Unary = fn(Box<Expr>) -> Expr;
/// The builder of a node with two operands.
type Binary = fn(Box<Expr>, Box<Expr>) -> Expr;
/// The builder of a quantifier from the name of its element, its list and
/// its test.
type Quantifier = fn(String, Box<Expr>, Box<Expr>) -> Expr;

/// The kinds of node with one operand, except `Neg`. `Neg` is apart because
/// validation does not let it hold the literal `None`.
const UNARY: [Unary; 16] = [
    Expr::Not,
    Expr::IsNone,
    Expr::IsNotNone,
    Expr::Len,
    Expr::Upper,
    Expr::Lower,
    Expr::Strip,
    Expr::LStrip,
    Expr::RStrip,
    Expr::IsDigit,
    Expr::IsAlpha,
    Expr::IsAlnum,
    Expr::IsSpace,
    Expr::IsUpper,
    Expr::IsLower,
    Expr::IsAscii,
];

/// The kinds of node with two operands that may hold the literal `None` on
/// either side.
pub const BINARY: [Binary; 4] = [Expr::Eq, Expr::Ne, Expr::StartsWith, Expr::EndsWith];

/// The orderings and the arithmetic operators. Validation refuses the
/// literal `None` on either side of them.
pub const NUMERIC: [Binary; 9] = [
    Expr::Lt,
    Expr::Le,
    Expr::Gt,
    Expr::Ge,
    Expr::Add,
    Expr::Sub,
    Expr::Mul,
    Expr::Div,
    Expr::Mod,
];

/// `in` and `not in`. Validation refuses the literal `None` on their right
/// side.
pub const MEMBERSHIP: [Binary; 2] = [Expr::In, Expr::NotIn];

/// The kinds of node that hold a list of operands.
const LISTS: [fn(Vec<Expr>) -> Expr; 3] = [Expr::List, Expr::And, Expr::Or];

/// The two quantifiers.
const QUANTIFIERS: [Quantifier; 2] = [
    |name, list, test| Expr::Any { name, list, test },
    |name, list, test| Expr::All { name, list, test },
];

/// The three pattern functions.
pub const PATTERNS: [fn(String, Box<Expr>) -> Expr; 3] = [
    |pattern, text| Expr::ReSearch { pattern, text },
    |pattern, text| Expr::ReMatch { pattern, text },
    |pattern, text| Expr::ReFullmatch { pattern, text },
];

/// Every variant of `TypeName`, the types that `isinstance` tests for.
const TYPES: [TypeName; 5] = [
    TypeName::Int,
    TypeName::Float,
    TypeName::Str,
    TypeName::Bool,
    TypeName::List,
];

/// The regex for a generated text: up to eight characters. `.` matches no
/// line break, so a text never has one.
const TEXT: &str = ".{0,8}";

/// Generates a pattern that compiles: one of a few realistic patterns, or a
/// short text of letters, digits and spaces.
pub fn pattern() -> BoxedStrategy<String> {
    prop_oneof![
        // An anchor, a class, a counted repeat and an alternation. The list
        // has no counted repeat of a Unicode class, such as `^\w{1,64}$`:
        // that compiles to megabytes and would slow down every test.
        select(vec!["^ab", "[0-9]+$", "x{2,4}", "cat|dog"]).prop_map(str::to_owned),
        // Letters, digits and spaces mean nothing special in a pattern, so
        // any such text compiles.
        "[a-zA-Z0-9 ]{0,8}",
    ]
    .boxed()
}

/// Generates every kind of node, in trees of at most three levels, with
/// finite floats, texts of at most eight characters and lists and paths of at
/// most three items. With `valid` set, the tree passes validation.
fn tree(valid: bool) -> BoxedStrategy<Expr> {
    // The small bounds let a failing case shrink to something a person can
    // read.
    let path = || {
        let segment = prop_oneof![
            TEXT.prop_map(Segment::Key),
            any::<u32>().prop_map(Segment::Index),
        ];
        vec(segment, 0..=3)
    };
    // The flags leave out `INFINITE`, `QUIET_NAN` and `SIGNALING_NAN`, so
    // every generated float is finite. JSON has no form for a NaN or an
    // infinity, and the JSON round trips of `tests/tree.rs` and
    // `tests/model.rs` compare for equality, where a NaN differs from itself.
    let float = {
        use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
        NEGATIVE | NORMAL | POSITIVE | SUBNORMAL | ZERO
    };
    let leaf = prop_oneof![
        Just(Expr::None),
        any::<bool>().prop_map(Expr::Bool),
        any::<i64>().prop_map(Expr::Int),
        float.prop_map(Expr::Float),
        TEXT.prop_map(Expr::Text),
        path().prop_map(Expr::Field),
        (TEXT, path()).prop_map(|(name, path)| Expr::Element { name, path }),
    ];
    // A depth of 2 recursions above the leaves gives the three levels. The
    // desired size 24 and the expected branch size 3 are choices that no
    // measurement backs.
    leaf.prop_recursive(2, 24, 3, move |inner| {
        let operand = || inner.clone().prop_map(Box::new);
        // An operand for the places where validation refuses the literal
        // `None`. With `valid` set it is never `None`. Without `valid` it can
        // be anything, so `expr()` still builds `x < None` for the JSON
        // tests.
        let not_none = || {
            inner
                .clone()
                .prop_filter("validation refuses None here", move |e| {
                    !valid || !matches!(e, Expr::None)
                })
                .prop_map(Box::new)
        };
        // The strategy does not hold the trees to the shapes the parser
        // builds. An `And` may hold an `And` or fewer than two operands, and
        // an `Element` may stand outside a quantifier that names it. No test
        // fails when a variant of `Expr` is missing from these arms, so
        // `engine/AGENTS.md` asks that a new kind of node is added here.
        prop_oneof![
            (select(UNARY.to_vec()), operand()).prop_map(|(make, a)| make(a)),
            not_none().prop_map(Expr::Neg),
            (select(BINARY.to_vec()), operand(), operand()).prop_map(|(make, a, b)| make(a, b)),
            (select(NUMERIC.to_vec()), not_none(), not_none()).prop_map(|(make, a, b)| make(a, b)),
            (select(MEMBERSHIP.to_vec()), operand(), not_none())
                .prop_map(|(make, a, b)| make(a, b)),
            (select(LISTS.to_vec()), vec(inner.clone(), 0..=3))
                .prop_map(|(make, items)| make(items)),
            (operand(), operand(), operand()).prop_map(|(test, then, otherwise)| Expr::If {
                test,
                then,
                otherwise
            }),
            (select(QUANTIFIERS.to_vec()), TEXT, operand(), operand())
                .prop_map(|(make, name, list, test)| make(name, list, test)),
            (operand(), vec(select(TYPES.to_vec()), 0..=3))
                .prop_map(|(value, types)| Expr::IsInstance { value, types }),
            // Without `valid` the pattern is any text. The JSON form does not
            // care whether a pattern compiles, and its tests should cover
            // that.
            (
                select(PATTERNS.to_vec()),
                if valid { pattern() } else { TEXT.boxed() },
                operand()
            )
                .prop_map(|(make, pattern, text)| make(pattern, text)),
        ]
    })
    .boxed()
}

/// Generates any tree. A pattern in it is any short text and may not compile.
pub fn expr() -> BoxedStrategy<Expr> {
    tree(false)
}

/// Generates a tree that passes validation.
pub fn valid_expr() -> BoxedStrategy<Expr> {
    tree(true)
}

/// Wraps `inner` in one random node. `inner` becomes one of that node's
/// operands, and `valid_expr()` fills in the rest, so only `inner` can make
/// validation fail.
fn frame(inner: Expr) -> BoxedStrategy<Expr> {
    let operand = || valid_expr().prop_map(Box::new);
    // The second operand of a two-operand node is never the literal `None`.
    // Some kinds would refuse it, and filtering it out for all of them lets
    // one arm build every kind.
    let other = || {
        valid_expr()
            .prop_filter("validation refuses None here", |e| !matches!(e, Expr::None))
            .prop_map(Box::new)
    };
    let held = Just(Box::new(inner));
    // `tree` builds `Neg` in an arm of its own, so it is added to `UNARY`
    // here.
    let unary = [&UNARY[..], &[Expr::Neg]].concat();
    let binary = [&BINARY[..], &NUMERIC[..], &MEMBERSHIP[..]].concat();
    // The arms cover every kind of node that has an operand, from the same
    // lists that `tree` uses. So the tests see a pointer through each kind.
    prop_oneof![
        (select(unary), held.clone()).prop_map(|(make, inner)| make(inner)),
        // `left` says which side `inner` goes on.
        (select(binary), held.clone(), other(), any::<bool>()).prop_map(
            |(make, inner, other, left)| {
                if left {
                    make(inner, other)
                } else {
                    make(other, inner)
                }
            }
        ),
        // `inner` goes in at a random position among up to two other items.
        (
            select(LISTS.to_vec()),
            held.clone(),
            vec(valid_expr(), 0..=2),
            any::<Index>()
        )
            .prop_map(|(make, inner, mut items, at)| {
                items.insert(at.index(items.len() + 1), *inner);
                make(items)
            }),
        // `at` picks where `inner` goes: the test, the first branch or the
        // `else` branch.
        (held.clone(), operand(), operand(), 0..3usize).prop_map(|(inner, a, b, at)| {
            let (test, then, otherwise) = match at {
                0 => (inner, a, b),
                1 => (a, inner, b),
                _ => (a, b, inner),
            };
            Expr::If {
                test,
                then,
                otherwise,
            }
        }),
        // `in_list` says whether `inner` is the list or the test.
        (
            select(QUANTIFIERS.to_vec()),
            TEXT,
            held.clone(),
            operand(),
            any::<bool>()
        )
            .prop_map(|(make, name, inner, other, in_list)| {
                if in_list {
                    make(name, inner, other)
                } else {
                    make(name, other, inner)
                }
            }),
        (held.clone(), vec(select(TYPES.to_vec()), 0..=3))
            .prop_map(|(value, types)| Expr::IsInstance { value, types }),
        (select(PATTERNS.to_vec()), pattern(), held)
            .prop_map(|(make, pattern, text)| make(pattern, text)),
    ]
    .boxed()
}

/// Buries `defect` under zero, one or two random nodes. Returns the tree and
/// how many nodes it put on top.
pub fn around(defect: Expr) -> BoxedStrategy<(Expr, usize)> {
    // Two nodes on top, plus the three levels of the other operands, stay
    // far below `MAX_DEPTH`. So only the defect can make validation fail.
    (0..=2usize)
        .prop_flat_map(move |n| {
            // Each round wraps the tree in one more node, so after `n` rounds
            // the defect sits `n` levels below the top.
            (0..n)
                .fold(Just(defect.clone()).boxed(), |tree, _| {
                    tree.prop_flat_map(frame).boxed()
                })
                .prop_map(move |tree| (tree, n))
        })
        .boxed()
}

/// A version 7 UUID from a generated timestamp and generated random bytes.
pub fn rule_id() -> impl Strategy<Value = Uuid> {
    // A version 7 UUID holds 48 bits of milliseconds.
    (0..1u64 << 48, any::<[u8; 10]>())
        .prop_map(|(millis, bytes)| uuid::Builder::from_unix_timestamp_millis(millis, &bytes))
        .prop_map(uuid::Builder::into_uuid)
}

/// A scope within the caps: 0 to `MAX_SCOPE_TAGS` tags, each key and value
/// of 1 to `MAX_TAG_LENGTH` characters.
pub fn scope() -> impl Strategy<Value = BTreeMap<String, String>> {
    // `.` matches no line break, so no generated tag holds one, although
    // `validate` accepts one.
    let text = || {
        proptest::string::string_regex(&format!(".{{1,{MAX_TAG_LENGTH}}}"))
            .expect("a valid pattern")
    };
    btree_map(text(), text(), 0..=MAX_SCOPE_TAGS)
}

/// Builds a rule with the id `rule_id` and the expression `e`. It only
/// observes, applies to every record, and has no title and no scope.
pub fn observing(rule_id: Uuid, e: Expr) -> Rule {
    Rule {
        rule_id,
        rule_title: None,
        action: Action::Observe,
        sample_rate: 1.0,
        scope: BTreeMap::new(),
        expression: e,
    }
}

/// A rule that `validate` accepts: a version 7 UUID as its id, a sample rate
/// in (0, 1], a scope within the caps and an expression from `valid_expr()`.
pub fn rule() -> impl Strategy<Value = Rule> {
    // The bound of 40 characters on a title is a choice that no measurement
    // backs.
    let title = proptest::option::of(".{0,40}");
    let action = select(vec![Action::Observe, Action::Drop, Action::Throw]);
    // `prop_oneof` draws `Just(1.0)` in about half of the cases, because a
    // draw from the range alone almost never gives 1.0, and that share is a
    // choice. The range starts at `f64::MIN_POSITIVE`, so 0 is never drawn.
    let sample_rate = prop_oneof![Just(1.0), f64::MIN_POSITIVE..=1.0];
    (rule_id(), title, action, sample_rate, scope(), valid_expr()).prop_map(
        |(rule_id, rule_title, action, sample_rate, scope, expression)| Rule {
            rule_id,
            rule_title,
            action,
            sample_rate,
            scope,
            expression,
        },
    )
}

/// The largest number of rows in a generated column. With this many rows, a
/// full chunk of `CHUNK_ROWS` rows can start at any row below `CHUNK_ROWS`,
/// and the 63 extra rows leave room for a last word that is only partly
/// filled. Miri interprets every generated row, so the number stays this
/// small.
pub const MAX_ROWS: usize = 2 * CHUNK_ROWS + 63;

/// Whether the bit of row `row` is set in the bitmap of a chunk.
pub fn bit(out: &Present, row: usize) -> bool {
    out[row / 64] >> (row % 64) & 1 == 1
}

/// Checks that `out` has one bit per row of `expected`, set where `expected`
/// is true, and no bit set past the end of `expected`.
pub fn assert_bitmap(out: &Present, expected: &[bool]) -> Result<(), TestCaseError> {
    for (row, want) in expected.iter().enumerate() {
        prop_assert_eq!(bit(out, row), *want, "row {}", row);
    }
    // The executor combines whole words, so a stray bit past the chunk would
    // count as an extra row.
    prop_assert!((expected.len()..CHUNK_WORDS * 64).all(|row| !bit(out, row)));
    Ok(())
}

/// Picks a chunk of a column of `len` rows from two generated indexes and
/// returns its start and its length. The first index picks any start from 0
/// to `len`, also one that is not a multiple of 64. The second picks a length
/// of at most `CHUNK_ROWS` that ends inside the column.
pub fn chunk_of(len: usize, (at, size): (Index, Index)) -> (usize, usize) {
    // A start at `len` gives an empty chunk at the end of the column, which
    // the loops must handle too.
    let start = at.index(len + 1);
    (start, size.index((len - start).min(CHUNK_ROWS) + 1))
}

/// Packs booleans into bytes, one bit each, lowest bit first, starting at bit
/// `offset`, the way a source bitmap such as Arrow's stores them. The bits
/// before `offset` are set, so a loop that reads them by mistake fails the
/// test.
pub fn packed(bools: &[bool], offset: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; (offset + bools.len()).div_ceil(8)];
    let bits = std::iter::repeat_n(true, offset).chain(bools.iter().copied());
    for (at, bit) in bits.enumerate() {
        bytes[at / 8] |= u8::from(bit) << (at % 8);
    }
    bytes
}

/// Generates one text for a text column: empty, of one to four bytes per
/// character, and on both sides of the 12 bytes that a view holds itself. No
/// text holds U+0000, which a fixed-width cell cannot keep at the end of a
/// value.
pub fn column_text() -> impl Strategy<Value = String> {
    // The class draws ASCII without U+0000 and one character each of two,
    // three and four bytes, so the ends of the texts fall after every width
    // of character. Up to 30 characters put texts below, at and above 12
    // bytes.
    "[\\x01-\\x7f\u{e9}\u{20ac}\u{1f600}]{0,30}"
}

/// Generates the texts of a text column of up to 39 rows.
pub fn texts() -> impl Strategy<Value = Vec<String>> {
    vec(column_text(), 0..40)
}

/// The type of the offsets of an offsets layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetType {
    I32,
    I64,
    U32,
    U64,
}

/// A layout of a text column, with the parts of its buffers that a reader
/// could get wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextLayout {
    /// Texts end to end, after `lead` bytes that belong to no text, as in a
    /// slice of a longer column. A reader that assumes a first offset of zero
    /// then reads the wrong bytes.
    Offsets { lead: usize, offset: OffsetType },
    /// Views of 16 bytes. A long text goes to the buffer of its row's parity,
    /// so the buffer number of a view is read and not assumed to be 0.
    Views,
    /// Pieces, where each text's piece is followed by `step - 1` pieces of
    /// other texts, as a field of a struct is followed by the struct's other
    /// fields.
    Pieces { step: usize },
    /// numpy `S` cells with the bytes of the longest text plus `slack`, and
    /// `step - 1` cells of other bytes between two rows, as in a stepped
    /// numpy view.
    Bytes { slack: usize, step: usize },
    /// numpy `U` cells with the characters of the longest text plus `slack`,
    /// and `step - 1` cells of other units between two rows.
    Utf32 { slack: usize, step: usize },
}

/// Generates any layout of a text column.
// A step of 1 lays the rows end to end, and a step of 2 or 3 puts other
// values between them. A slack of 0 lets the longest text fill its cell
// with no zero after it.
pub fn text_layout() -> impl Strategy<Value = TextLayout> {
    let offset = select(vec![
        OffsetType::I32,
        OffsetType::I64,
        OffsetType::U32,
        OffsetType::U64,
    ]);
    prop_oneof![
        (0usize..5, offset).prop_map(|(lead, offset)| TextLayout::Offsets { lead, offset }),
        Just(TextLayout::Views),
        (1usize..4).prop_map(|step| TextLayout::Pieces { step }),
        (0usize..3, 1usize..4).prop_map(|(slack, step)| TextLayout::Bytes { slack, step }),
        (0usize..3, 1usize..4).prop_map(|(slack, step)| TextLayout::Utf32 { slack, step }),
    ]
}

/// The buffers of a text column in one layout, which own every byte that the
/// column reads. `lend` borrows them for a column.
#[derive(Debug, Clone)]
pub struct TextBuffers {
    rows: usize,
    buffers: Buffers,
}

/// The buffers of each layout.
#[derive(Debug, Clone)]
enum Buffers {
    // The ends of the texts in each of the four offset types, because
    // `offsets` gives all four layouts to the column tests. `offset` names
    // the one that `text` takes.
    Offsets {
        bytes: Vec<u8>,
        i32s: Vec<i32>,
        i64s: Vec<i64>,
        u32s: Vec<u32>,
        u64s: Vec<u64>,
        offset: OffsetType,
    },
    Views {
        views: Vec<View>,
        long: [Vec<u8>; 2],
    },
    // A piece borrows its text, so `lend` makes the pieces.
    Pieces {
        texts: Vec<String>,
        step: usize,
    },
    Bytes {
        bytes: Vec<u8>,
        width: usize,
        step: usize,
    },
    Utf32 {
        units: Vec<u32>,
        width: usize,
        step: usize,
    },
}

/// Converts the ends of the texts to the offset type `O`.
fn offsets_as<O: TryFrom<usize>>(ends: &[usize]) -> Vec<O> {
    ends.iter()
        .map(|n| {
            // `expect` needs a `Debug` error, which `O::Error` need not be,
            // so the error is dropped.
            O::try_from(*n)
                .map_err(|_| ())
                .expect("every end fits the offset type")
        })
        .collect()
}

impl TextBuffers {
    /// Builds the buffers of a column of `texts` in `layout`.
    pub fn new(texts: &[String], layout: TextLayout) -> Self {
        let buffers = match layout {
            TextLayout::Offsets { lead, offset } => {
                let mut bytes = vec![b'x'; lead];
                let mut ends = vec![lead];
                for text in texts {
                    bytes.extend_from_slice(text.as_bytes());
                    ends.push(bytes.len());
                }
                Buffers::Offsets {
                    bytes,
                    i32s: offsets_as(&ends),
                    i64s: offsets_as(&ends),
                    u32s: offsets_as(&ends),
                    u64s: offsets_as(&ends),
                    offset,
                }
            }
            TextLayout::Views => {
                let mut long = [Vec::new(), Vec::new()];
                let mut views = Vec::new();
                for (row, text) in texts.iter().enumerate() {
                    let view = if text.len() <= INLINE {
                        View::inline(text)
                    } else {
                        let buffer = &mut long[row % 2];
                        let offset = u32::try_from(buffer.len()).expect("the buffer is small");
                        buffer.extend_from_slice(text.as_bytes());
                        View::outside(text, u32::try_from(row % 2).expect("0 or 1"), offset)
                    };
                    views.push(view.expect("the text fits a view"));
                }
                Buffers::Views { views, long }
            }
            TextLayout::Pieces { step } => Buffers::Pieces {
                texts: texts.to_vec(),
                step,
            },
            TextLayout::Bytes { slack, step } => {
                let width = texts.iter().map(String::len).max().unwrap_or(0) + slack;
                let mut bytes = Vec::new();
                for text in texts {
                    let start = bytes.len();
                    bytes.extend_from_slice(text.as_bytes());
                    bytes.resize(start + width, 0);
                    bytes.resize(start + step * width, b'y');
                }
                Buffers::Bytes { bytes, width, step }
            }
            TextLayout::Utf32 { slack, step } => {
                let chars = texts
                    .iter()
                    .map(|t| t.chars().map(u32::from).collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                let width = chars.iter().map(Vec::len).max().unwrap_or(0) + slack;
                let mut units = Vec::new();
                for text in &chars {
                    let start = units.len();
                    units.extend_from_slice(text);
                    units.resize(start + width, 0);
                    units.resize(start + step * width, u32::from('y'));
                }
                Buffers::Utf32 { units, width, step }
            }
        };
        Self {
            rows: texts.len(),
            buffers,
        }
    }

    /// The number of texts.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Borrows the buffers for a column.
    pub fn lend(&self) -> LentTexts<'_> {
        let pieces = match &self.buffers {
            Buffers::Pieces { texts, step } => texts
                .iter()
                .flat_map(|t| {
                    std::iter::once(Piece::new(t))
                        .chain(std::iter::repeat_n(Piece::new("x"), step - 1))
                })
                .collect(),
            _ => Vec::new(),
        };
        let long = match &self.buffers {
            Buffers::Views { long, .. } => [&long[0][..], &long[1][..]],
            _ => [&[][..], &[][..]],
        };
        LentTexts {
            buffers: self,
            pieces,
            long,
        }
    }

    /// The units and the width of the cells of a `Bytes` layout, and `None`
    /// for any other layout.
    pub fn byte_cells(&self) -> Option<(Units<'_, u8>, usize)> {
        match &self.buffers {
            // SAFETY: every cell holds a text, which is UTF-8, padded with
            // zeros, and the bytes between the cells are ASCII.
            Buffers::Bytes { bytes, width, .. } => Some((unsafe { Units::utf8(bytes) }, *width)),
            _ => None,
        }
    }
}

/// The buffers of a text column, borrowed for a column, with the pieces and
/// the list of buffers that a layout borrows in turn.
pub struct LentTexts<'a> {
    buffers: &'a TextBuffers,
    pieces: Vec<Piece<'a>>,
    long: [&'a [u8]; 2],
}

impl LentTexts<'_> {
    /// Returns the layout of the column.
    pub fn text(&self) -> Text<'_> {
        let rows = self.buffers.rows;
        match &self.buffers.buffers {
            Buffers::Offsets { offset, .. } => {
                let [i32s, i64s, u32s, u64s] = self.offsets().expect("an offsets layout");
                Text::Offsets(match offset {
                    OffsetType::I32 => i32s,
                    OffsetType::I64 => i64s,
                    OffsetType::U32 => u32s,
                    OffsetType::U64 => u64s,
                })
            }
            // SAFETY: each long view names the buffer and the offset that its
            // text was copied to, and the texts are UTF-8.
            Buffers::Views { views, .. } => Text::Views(unsafe { Views::new(views, &self.long) }),
            Buffers::Pieces { step, .. } => {
                // SAFETY: the pointer is the vector's, the stride is `step`
                // pieces in bytes, and the count is the number of texts, each
                // of which has `step` pieces in the vector.
                let run = unsafe {
                    Strided::from_raw(self.pieces.as_ptr(), step * size_of::<Piece<'_>>(), rows)
                };
                Text::Pieces(run)
            }
            Buffers::Bytes { width, step, .. } => {
                // `byte_cells` holds the one promise that these bytes are
                // UTF-8.
                let (units, _) = self.buffers.byte_cells().expect("a bytes layout");
                Text::Bytes(
                    Fixed::new(*width, step * width, rows, units)
                        .expect("the cells hold every row"),
                )
            }
            Buffers::Utf32 { units, width, step } => Text::Utf32(
                Fixed::new(*width, step * width, rows, Units::utf32(units))
                    .expect("the cells hold every row"),
            ),
        }
    }

    /// Returns the four offsets layouts of an `Offsets` layout, one per
    /// offset type, in the order `i32`, `i64`, `u32` and `u64`, and `None` for
    /// any other layout.
    pub fn offsets(&self) -> Option<[AnyOffsets<'_, Utf8<'_>>; 4]> {
        let Buffers::Offsets {
            bytes,
            i32s,
            i64s,
            u32s,
            u64s,
            ..
        } = &self.buffers.buffers
        else {
            return None;
        };
        // SAFETY: every range between two ends is one of the texts, which are
        // UTF-8.
        let utf8 = unsafe { Utf8::new(bytes) };
        // SAFETY: the offsets are the ends of the texts, in order, and the
        // last is the length of the bytes.
        Some(unsafe {
            [
                AnyOffsets::I32(Offsets::new(i32s, utf8)),
                AnyOffsets::I64(Offsets::new(i64s, utf8)),
                AnyOffsets::U32(Offsets::new(u32s, utf8)),
                AnyOffsets::U64(Offsets::new(u64s, utf8)),
            ]
        })
    }
}
