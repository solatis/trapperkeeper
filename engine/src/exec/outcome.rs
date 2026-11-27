//! What a run gives: the outcome of each rule for each record (ADR-0006).

/// Why a rule did not apply to a record (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fault {
    /// Two values of types that do not compare.
    Incompatible,
    /// An ordering of values that are not numbers, such as two booleans.
    Unordered,
    /// A value that is not a boolean where a condition is expected.
    NotBoolean,
}

/// What a rule gave for one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Matched,
    /// The expression gives false or unknown, and the rule does not match.
    /// Unknown is the null of a boolean. A comparison gives it when a value
    /// is a null (ADR-0005).
    NotMatched,
    NotEvaluated(Fault),
}

/// What a run gave for each rule, in the order of the rule set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcomes {
    rows: usize,
    rules: Vec<RuleOutcome>,
}

/// What a run gave for one rule over the records of a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOutcome {
    rule_id: uuid::Uuid,
    rows: usize,
    // Record `i` is bit `i % 64` of word `i / 64`, as in the bitmap of a
    // chunk.
    matched: Vec<u64>,
    faulted: Vec<u64>,
    // The faulted records in ascending order, each with its fault.
    faults: Vec<(usize, Fault)>,
}

impl Outcomes {
    /// No outcome yet for a batch of `rows` records and the rules `rule_ids`.
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

    /// The outcome of each rule, for a run that appends to them.
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

    /// The bitmap of the matched records.
    #[must_use]
    pub fn matched(&self) -> &[u64] {
        &self.matched
    }

    /// The bitmap of the records that the rule did not apply to.
    #[must_use]
    pub fn faulted(&self) -> &[u64] {
        &self.faulted
    }

    /// The faulted records in ascending order, each with its fault.
    #[must_use]
    pub fn faults(&self) -> &[(usize, Fault)] {
        &self.faults
    }

    /// The outcome for record `row`.
    ///
    /// # Panics
    ///
    /// Panics when `row` is not below the number of records in the batch.
    #[must_use]
    pub fn outcome(&self, row: usize) -> Outcome {
        assert!(row < self.rows, "record {row} of a batch of {}", self.rows);
        let bit = |words: &[u64]| words[row / 64] >> (row % 64) & 1 == 1;
        // A faulted record is never true, so no record is in both bitmaps.
        if bit(&self.matched) {
            Outcome::Matched
        } else if bit(&self.faulted) {
            // `faults` is in ascending order of record, so a binary search
            // finds the record's fault.
            let at = self
                .faults
                .binary_search_by_key(&row, |&(r, _)| r)
                .expect("a faulted record has its fault in the list");
            Outcome::NotEvaluated(self.faults[at].1)
        } else {
            Outcome::NotMatched
        }
    }

    /// Appends the matched and the faulted records of one chunk, whose first
    /// record is `start` and which holds `rows` records. `start` is the number
    /// of records appended before, and a chunk other than the last holds a
    /// multiple of 64 records. `fault_of` takes the row of a faulted record in
    /// the chunk and gives its fault.
    pub(crate) fn append(
        &mut self,
        matched: &[u64],
        faulted: &[u64],
        start: usize,
        rows: usize,
        mut fault_of: impl FnMut(usize) -> Fault,
    ) {
        // A chunk starts at a multiple of 64 records, so its words follow the
        // words of the chunk before it.
        let words = rows.div_ceil(64);
        // The loops set no bit from `rows` on, so the last word adds no
        // record past the chunk.
        self.matched.extend_from_slice(&matched[..words]);
        self.faulted.extend_from_slice(&faulted[..words]);
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
