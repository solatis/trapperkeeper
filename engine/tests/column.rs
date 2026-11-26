//! The properties of the column description. Each one builds a description
//! from generated data and checks that a reader or a loop gives that data
//! back. The properties keep proptest's default case count, so that
//! `PROPTEST_CASES` can lower it under Miri.

use std::mem::size_of;

use proptest::collection::vec;
use proptest::prelude::*;
use trapperkeeper_engine::column::{Bits, Strided};

/// Packs booleans into bytes from bit `offset` on, lowest bit first, as a
/// source bitmap holds them. The bits before `offset` are set, so a loop that
/// reads them shows.
fn packed(bools: &[bool], offset: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; (offset + bools.len()).div_ceil(8)];
    let bits = std::iter::repeat_n(true, offset).chain(bools.iter().copied());
    for (at, bit) in bits.enumerate() {
        bytes[at / 8] |= u8::from(bit) << (at % 8);
    }
    bytes
}

proptest! {
    // `failure_persistence: None` turns off proptest's regression file, so a
    // failing run writes no untracked file into the working tree.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A run reads back the values of the slice it was built from, and a
    /// strided run reads every `step`-th value.
    #[test]
    fn a_run_reads_back_its_values(xs in vec(any::<i64>(), 0..300), step in 1usize..4) {
        let run = Strided::from_slice(&xs);
        prop_assert_eq!(run.len(), xs.len());
        prop_assert!((0..xs.len()).all(|row| run.get(row) == xs[row]));
        // A run from a slice is contiguous.
        prop_assert_eq!(run.as_slice(), Some(&xs[..]));

        let every = xs.iter().step_by(step).copied().collect::<Vec<_>>();
        // SAFETY: the pointer is the slice's, the stride is `step` values in
        // bytes, and the count is the number of `step`-th values that the
        // slice holds, so every value read lies inside the slice.
        let strided = unsafe { Strided::from_raw(xs.as_ptr(), step * size_of::<i64>(), every.len()) };
        prop_assert!((0..every.len()).all(|row| strided.get(row) == every[row]));
        // A run with a step above 1 is not contiguous.
        prop_assert_eq!(strided.as_slice().is_some(), step == 1);
    }

    /// A run of bits reads back the booleans it was packed from, at any bit
    /// offset, and `Bits::new` refuses a buffer that is too short.
    #[test]
    fn bits_read_back_their_booleans(bools in vec(any::<bool>(), 0..300), offset in 0usize..17) {
        let bytes = packed(&bools, offset);
        let bits = Bits::new(&bytes, offset, bools.len()).expect("the buffer holds the bits");
        prop_assert_eq!(bits.len(), bools.len());
        prop_assert!((0..bools.len()).all(|row| bits.get(row) == bools[row]));
        // `packed` makes the shortest buffer, so one byte less cannot hold the
        // bits whenever there is a byte.
        if let Some(short) = bytes.len().checked_sub(1) {
            prop_assert!(Bits::new(&bytes[..short], offset, bools.len()).is_none());
        }
    }
}
