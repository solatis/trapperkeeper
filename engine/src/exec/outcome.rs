//! What a run returns: for each rule and each record, whether the rule
//! matched, did not match, or could not be evaluated (ADR-0006).

/// Why a rule could not be evaluated on a record (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fault {
    /// Two values of types that cannot be compared, as in `"a" == 5`.
    Incompatible,
    /// `<`, `<=`, `>` or `>=` on values that are not numbers, as in
    /// `True < False`.
    Unordered,
    /// A value that is not a boolean used as a condition, as `x` is in
    /// `x and y` when `x` holds a number.
    NotBoolean,
}

/// What one rule gave for one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The rule's expression is true.
    Matched,
    /// The rule's expression is false or unknown. It is unknown when a value
    /// it needs is missing, as for `x > 5` where `x` is `None` (ADR-0005).
    NotMatched,
    /// The rule could not be evaluated on the record, for the given reason.
    NotEvaluated(Fault),
}

/// What a run gave for each rule over the records of one batch, in the
/// order of the rule set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcomes {
    rows: usize,
    rules: Vec<RuleOutcome>,
}

/// What one rule gave for each record of a batch: a bitmap of the matched
/// records, a bitmap of the records it could not evaluate, and the fault of
/// each of those.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOutcome {
    rule_id: uuid::Uuid,
    rows: usize,
    // Record `i` is bit `i % 64` of word `i / 64`, the same layout as the
    // executor's bitmaps, so a chunk's words are appended as they are.
    matched: Vec<u64>,
    faulted: Vec<u64>,
    // The faulted records in ascending order, each with its fault.
    faults: Vec<(usize, Fault)>,
}

impl Outcomes {
    /// Creates empty outcomes for a batch of `rows` records and the rules
    /// `rule_ids`. `run` fills them chunk by chunk.
    pub(crate) fn new(rows: usize, rule_ids: impl IntoIterator<Item = uuid::Uuid>) -> Self {
        let rules = rule_ids
            .into_iter()
            .map(|rule_id| RuleOutcome {
                rule_id,
                rows,
                matched: Vec::with_capacity(rows.div_ceil(64)),
                faulted: Vec::with_capacity(rows.div_ceil(64)),
                faults: Vec::new(),
            })
            .collect();
        Self { rows, rules }
    }

    /// The outcome of each rule, in the order of the rule set.
    #[must_use]
    pub fn rules(&self) -> &[RuleOutcome] {
        &self.rules
    }

    /// The number of records in the batch.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The outcome of each rule, for `run` to append to.
    pub(crate) fn rules_mut(&mut self) -> &mut [RuleOutcome] {
        &mut self.rules
    }
}

impl RuleOutcome {
    /// The rule's id.
    #[must_use]
    pub fn rule_id(&self) -> uuid::Uuid {
        self.rule_id
    }

    /// The matched records, one bit per record: record `i` is bit `i % 64`
    /// of word `i / 64`.
    #[must_use]
    pub fn matched(&self) -> &[u64] {
        &self.matched
    }

    /// The records that the rule could not evaluate, in the same layout as
    /// `matched`.
    #[must_use]
    pub fn faulted(&self) -> &[u64] {
        &self.faulted
    }

    /// The faulted records in ascending order, each with its fault.
    #[must_use]
    pub fn faults(&self) -> &[(usize, Fault)] {
        &self.faults
    }

    /// Returns the outcome of the rule for record `row`.
    ///
    /// # Panics
    ///
    /// Panics when `row` is not below the number of records in the batch.
    #[must_use]
    pub fn outcome(&self, row: usize) -> Outcome {
        assert!(row < self.rows, "record {row} of a batch of {}", self.rows);
        let bit = |words: &[u64]| words[row / 64] >> (row % 64) & 1 == 1;
        // A faulted record is never true, so it is never in both bitmaps and
        // the order of the two tests does not matter.
        if bit(&self.matched) {
            Outcome::Matched
        } else if bit(&self.faulted) {
            // `faults` is sorted by record, so a binary search finds it.
            let at = self
                .faults
                .binary_search_by_key(&row, |&(r, _)| r)
                .expect("a faulted record has its fault in the list");
            Outcome::NotEvaluated(self.faults[at].1)
        } else {
            Outcome::NotMatched
        }
    }

    /// Appends the result of one chunk: the bitmaps `matched` and `faulted`
    /// of a chunk of `rows` records that starts at record `start`.
    /// `fault_of` returns the fault of a faulted row, given its row number in
    /// the chunk. Chunks must be appended in order, and every chunk but the
    /// last must hold a multiple of 64 records.
    pub(crate) fn append(
        &mut self,
        matched: &[u64],
        faulted: &[u64],
        start: usize,
        rows: usize,
        mut fault_of: impl FnMut(usize) -> Fault,
    ) {
        // Every chunk before this one held a multiple of 64 records, so this
        // chunk starts at a word boundary and its words can be appended as
        // they are.
        let words = rows.div_ceil(64);
        // The bits past the chunk's last row are 0, so the last word adds no
        // record that does not exist.
        self.matched.extend_from_slice(&matched[..words]);
        self.faulted.extend_from_slice(&faulted[..words]);
        // Visits each set bit of `faulted`, lowest first, so the faults are
        // appended in ascending order of record.
        for (k, &word) in faulted[..words].iter().enumerate() {
            let mut rest = word;
            while rest != 0 {
                let row = 64 * k + rest.trailing_zeros() as usize;
                self.faults.push((start + row, fault_of(row)));
                // Clears the lowest set bit.
                rest &= rest - 1;
            }
        }
    }
}
