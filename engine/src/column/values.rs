//! Where the values of a column lie in a source's memory. ADR-0036 gives the
//! reasons for the layouts.

use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::ptr::NonNull;

/// Values of one fixed-width type in a source's memory. The values lie
/// `stride` bytes apart, and the run borrows the memory for `'a`.
#[derive(Debug, Clone, Copy)]
pub struct Strided<'a, T> {
    // The address of the first value. It is never null, and an empty run
    // holds a dangling address that nothing reads.
    first: NonNull<T>,
    // The distance in bytes from one value to the next. It equals the size of
    // `T` in a plain array and is larger for a field in a slice of structs.
    stride: usize,
    len: usize,
    // Ties the run to the lifetime of the source's memory, which the raw
    // pointer alone does not carry.
    borrow: PhantomData<&'a T>,
}

impl<'a, T: Copy> Strided<'a, T> {
    /// The values of a slice, one after the other.
    #[must_use]
    pub fn from_slice(values: &'a [T]) -> Self {
        Self {
            first: NonNull::from(values).cast(),
            stride: size_of::<T>(),
            len: values.len(),
            borrow: PhantomData,
        }
    }

    /// A run over memory that Rust did not allocate, or over values a stride
    /// apart.
    ///
    /// # Safety
    ///
    /// `first` is not null and is aligned for `T`. `stride` is a positive
    /// multiple of the alignment of `T`. For every `i` below `len`, the bytes
    /// of one `T` at `first` plus `i * stride` are readable and hold a valid
    /// `T`, and nothing writes them during `'a`.
    #[must_use]
    pub unsafe fn from_raw(first: *const T, stride: usize, len: usize) -> Self {
        // These check the parts of the contract that can be checked, so that
        // an adapter's mistake shows in a debug build and under Miri.
        debug_assert!(!first.is_null(), "the first value is at a null address");
        debug_assert!(first.is_aligned(), "the first value is not aligned");
        debug_assert!(
            stride > 0 && stride.is_multiple_of(align_of::<T>()),
            "the stride is not a positive multiple of the alignment"
        );
        Self {
            // SAFETY: the caller promises that `first` is not null.
            first: unsafe { NonNull::new_unchecked(first.cast_mut()) },
            stride,
            len,
            borrow: PhantomData,
        }
    }

    /// The number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the run holds no value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The values as a slice, when they lie one after the other. `None` for a
    /// run with a larger stride.
    #[must_use]
    pub fn as_slice(&self) -> Option<&'a [T]> {
        // A stride equal to the size of `T` leaves no bytes between the
        // values, so the run is a slice.
        if self.stride != size_of::<T>() {
            return None;
        }
        // SAFETY: the constructor's contract makes `len` values readable from
        // `first` for `'a`, and with this stride they are adjacent.
        Some(unsafe { std::slice::from_raw_parts(self.first.as_ptr(), self.len) })
    }

    /// The value at `row`.
    ///
    /// # Panics
    ///
    /// Panics when `row` is not below `len`.
    #[must_use]
    pub fn get(&self, row: usize) -> T {
        // The read below trusts this bound, and the bound is what makes this
        // function safe to call.
        assert!(row < self.len, "row {row} of a run of {}", self.len);
        // SAFETY: `row` is below `len`, and the constructor's contract makes
        // the value at `first` plus `row * stride` bytes readable and aligned.
        unsafe { self.first.byte_add(row * self.stride).read() }
    }
}
