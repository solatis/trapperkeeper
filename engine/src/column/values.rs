//! Where the values of a column lie in a source's memory. ADR-0036 gives the
//! reasons for the layouts.

use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::ptr::NonNull;
use std::simd::Simd;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::num::{SimdFloat, SimdInt, SimdUint};

use super::Kind;

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
        // These check the parts of the contract that can be checked, so that a
        // caller that breaks it fails in a debug build and under Miri.
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
        // The unchecked read below trusts this bound, so `get` stays sound for
        // any `row`.
        assert!(row < self.len, "row {row} of a run of {}", self.len);
        // SAFETY: the assertion above keeps `row` below `len`.
        unsafe { self.get_unchecked(row) }
    }

    /// The same run, read as values of type `U`.
    ///
    /// # Safety
    ///
    /// `U` has the size and the alignment of `T`, and every bit pattern of a
    /// `T` is a valid `U`.
    pub(crate) unsafe fn cast<U>(self) -> Strided<'a, U> {
        // This checks the size and the alignment of the contract. No check
        // can show that every bit pattern of a `T` is a valid `U`.
        debug_assert!(size_of::<U>() == size_of::<T>() && align_of::<U>() == align_of::<T>());
        Strided {
            first: self.first.cast(),
            stride: self.stride,
            len: self.len,
            borrow: PhantomData,
        }
    }

    /// The value at `row`, without the bound check.
    ///
    /// # Safety
    ///
    /// `row` is below `len`.
    pub(crate) unsafe fn get_unchecked(&self, row: usize) -> T {
        // SAFETY: the caller keeps `row` below `len`, and the constructor's
        // contract makes the value at `first` plus `row * stride` bytes
        // readable. The read is aligned, because the stride is a multiple of
        // the alignment of `T`.
        unsafe { self.first.byte_add(row * self.stride).read() }
    }
}

/// A run of bits in a source's memory, such as the validity bitmap of a column
/// or a bit-packed boolean column. Bit `i` of the run is bit `(offset + i) % 8`
/// of byte `(offset + i) / 8`, counted from the least significant bit.
#[derive(Debug, Clone, Copy)]
pub struct Bits<'a> {
    bytes: &'a [u8],
    // Slicing a column does not copy its bitmap, so the run of a slice starts
    // at any bit of the source's buffer.
    offset: usize,
    len: usize,
}

impl<'a> Bits<'a> {
    /// `len` bits of `bytes` from bit `offset` on. `None` when `bytes` is too
    /// short to hold them.
    #[must_use]
    pub fn new(bytes: &'a [u8], offset: usize, len: usize) -> Option<Self> {
        // The checked add refuses an offset near the largest `usize`, where
        // the sum would wrap and pass the comparison.
        let end = offset.checked_add(len)?;
        (end.div_ceil(8) <= bytes.len()).then_some(Self { bytes, offset, len })
    }

    /// The number of bits.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the run holds no bit.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bit at `row`.
    ///
    /// # Panics
    ///
    /// Panics when `row` is not below `len`.
    #[must_use]
    pub fn get(&self, row: usize) -> bool {
        assert!(row < self.len, "bit {row} of a run of {}", self.len);
        let at = self.offset + row;
        self.bytes[at / 8] >> (at % 8) & 1 == 1
    }

    /// The bytes that hold the bits and the bit at which the run starts.
    pub(crate) fn parts(&self) -> (&'a [u8], usize) {
        (self.bytes, self.offset)
    }
}

/// The values of an int column: one of the eight fixed-width int types.
#[derive(Debug, Clone, Copy)]
pub enum Ints<'a> {
    I8(Strided<'a, i8>),
    I16(Strided<'a, i16>),
    I32(Strided<'a, i32>),
    I64(Strided<'a, i64>),
    U8(Strided<'a, u8>),
    U16(Strided<'a, u16>),
    U32(Strided<'a, u32>),
    U64(Strided<'a, u64>),
}

/// The values of a float column.
#[derive(Debug, Clone, Copy)]
pub enum Floats<'a> {
    // Nightly's half-precision type, which numpy and pandas columns hold.
    F16(Strided<'a, f16>),
    F32(Strided<'a, f32>),
    F64(Strided<'a, f64>),
}

/// The values of a boolean column.
#[derive(Debug, Clone, Copy)]
pub enum Booleans<'a> {
    // One byte per value, as numpy holds booleans. A byte that is not zero is
    // true, so no byte of the source's memory is read as a Rust `bool`, for
    // which any byte but 0 and 1 is undefined behavior.
    Bytes(Strided<'a, u8>),
    // One bit per value, as Polars and Arrow data hold booleans.
    Bits(Bits<'a>),
}

impl Ints<'_> {
    /// The number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::I8(v) => v.len(),
            Self::I16(v) => v.len(),
            Self::I32(v) => v.len(),
            Self::I64(v) => v.len(),
            Self::U8(v) => v.len(),
            Self::U16(v) => v.len(),
            Self::U32(v) => v.len(),
            Self::U64(v) => v.len(),
        }
    }

    /// Whether the column holds no value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Floats<'_> {
    /// The number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::F16(v) => v.len(),
            Self::F32(v) => v.len(),
            Self::F64(v) => v.len(),
        }
    }

    /// Whether the column holds no value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Booleans<'_> {
    /// The number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Bytes(v) => v.len(),
            Self::Bits(v) => v.len(),
        }
    }

    /// Whether the column holds no value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub(super) mod sealed {
    use std::simd::{Simd, SimdCast, SimdElement};

    use super::super::Kind;
    use super::Strided;

    /// The part of `Number` that only this crate can name, so no crate
    /// outside the engine implements `Number` for a type of its own.
    // `Send`, `Sync` and `'static` let a loop that holds a value of the type
    // be shared between threads. `Default` gives the zero that a test of the
    // sign compares with, and `PartialOrd` lets the scalar arm of a loop
    // compare two values.
    pub trait Number:
        Copy + Default + PartialOrd + Send + Sync + 'static + SimdElement + SimdCast
    {
        /// Whether the type is a float. The executor picks the type in which
        /// two number columns compare from this and from `SIGNED`.
        const FLOAT: bool;

        /// Whether the type holds negative values.
        const SIGNED: bool;

        /// The values of `kind` when it is a column of this type.
        fn values<'a>(kind: &Kind<'a>) -> Option<Strided<'a, Self>>;

        /// For 64 lanes, whether `left` is below `right`, with lane `i` in
        /// bit `i`.
        fn lt_lanes(left: Simd<Self, 64>, right: Simd<Self, 64>) -> u64;

        /// For 64 lanes, whether `left` equals `right`, with lane `i` in bit
        /// `i`.
        fn eq_lanes(left: Simd<Self, 64>, right: Simd<Self, 64>) -> u64;

        /// Converts each of 64 lanes to `C`, as Rust's `as` converts a value.
        fn convert_lanes<C: super::Number>(lanes: Simd<Self, 64>) -> Simd<C, 64>;

        /// Converts the value to `C`, as one lane of `convert_lanes` does.
        fn convert<C: super::Number>(self) -> C;
    }
}

/// A number type that a column can hold.
// The trait is sealed, because the engine owns the closed list of number
// types and no other crate extends it (ADR-0036).
pub trait Number: sealed::Number {}

// Writes the impl of `Number` for one number type. `$kind::$variant` is the
// variant of `Ints` or `Floats` that holds a column of the type.
macro_rules! number {
    ($t:ty, $kind:ident::$variant:ident, $float:expr, $signed:expr) => {
        impl sealed::Number for $t {
            const FLOAT: bool = $float;
            const SIGNED: bool = $signed;

            fn values<'a>(kind: &Kind<'a>) -> Option<Strided<'a, Self>> {
                match kind {
                    Kind::$kind($kind::$variant(v), _) => Some(*v),
                    _ => None,
                }
            }

            fn lt_lanes(left: Simd<Self, 64>, right: Simd<Self, 64>) -> u64 {
                left.simd_lt(right).to_bitmask()
            }

            fn eq_lanes(left: Simd<Self, 64>, right: Simd<Self, 64>) -> u64 {
                left.simd_eq(right).to_bitmask()
            }

            fn convert_lanes<C: Number>(lanes: Simd<Self, 64>) -> Simd<C, 64> {
                lanes.cast::<C>()
            }

            fn convert<C: Number>(self) -> C {
                // Rust's `as` takes no generic target type, so a cast of one
                // lane stands in for it and converts as `as` does.
                Simd::<Self, 1>::splat(self).cast::<C>()[0]
            }
        }

        impl Number for $t {}
    };
}

// Writes everything that names each number type, from the one list below:
// the impls of `Number`, the enum `NumberType`, and the macros `with_int!`,
// `with_float!` and `with_number!`. The first token is a `$`, which the
// macros that this macro writes need for their own variables, because a
// `$` inside a macro's body would otherwise belong to this macro.
macro_rules! numbers {
    (
        $d:tt
        ints { $($it:ty => $iv:ident, $signed:expr;)* }
        floats { $($ft:ty => $fv:ident;)* }
    ) => {
        $(number!($it, Ints::$iv, false, $signed);)*
        $(number!($ft, Floats::$fv, true, true);)*

        /// The type of the values of a number column.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum NumberType {
            $($iv,)*
            $($fv,)*
        }

        impl NumberType {
            /// Whether the type is a float type.
            pub(crate) fn is_float(self) -> bool {
                matches!(self, $(Self::$fv)|*)
            }
        }

        impl Ints<'_> {
            /// The type of the values.
            pub(crate) fn number_type(&self) -> NumberType {
                match self {
                    $(Self::$iv(_) => NumberType::$iv,)*
                }
            }
        }

        impl Floats<'_> {
            /// The type of the values.
            pub(crate) fn number_type(&self) -> NumberType {
                match self {
                    $(Self::$fv(_) => NumberType::$fv,)*
                }
            }
        }

        /// Turns a `NumberType` that is only known at run time into a Rust
        /// type that the compiler sees. `with_int!(n, |T| body)` expands to
        /// a `match` on `n` with one arm per int type, and each arm compiles
        /// `body` with `T` set to that type, such as `i8` or `u64`. Code that
        /// is generic over `T` thus gets one copy per type, each with the
        /// SIMD instructions of its type. A float type panics.
        macro_rules! with_int {
            ($d n:expr, |$d T:ident| $d body:expr) => {
                match $d n {
                    $($crate::column::NumberType::$iv => {
                        type $d T = $it;
                        $d body
                    })*
                    _ => unreachable!("the caller checked that the field holds ints"),
                }
            };
        }
        pub(crate) use with_int;

        /// `with_int!` for the float types: `with_float!(n, |T| body)` compiles
        /// `body` once with `T` as `f16`, once as `f32` and once as `f64`,
        /// and runs the copy for `n`. An int type panics.
        macro_rules! with_float {
            ($d n:expr, |$d T:ident| $d body:expr) => {
                match $d n {
                    $($crate::column::NumberType::$fv => {
                        type $d T = $ft;
                        $d body
                    })*
                    _ => unreachable!("the caller checked that the field holds floats"),
                }
            };
        }
        pub(crate) use with_float;

        /// `with_int!` for every number type, int or float.
        macro_rules! with_number {
            ($d n:expr, |$d T:ident| $d body:expr) => {
                match $d n {
                    $($crate::column::NumberType::$iv => {
                        type $d T = $it;
                        $d body
                    })*
                    $($crate::column::NumberType::$fv => {
                        type $d T = $ft;
                        $d body
                    })*
                }
            };
        }
        pub(crate) use with_number;
    };
}

// The number types that a column can hold. Each int type says whether it is
// signed. The variant of `Ints` or `Floats` that holds a column of the type
// has the name of its `NumberType`.
numbers! {
    $
    ints {
        i8 => I8, true;
        i16 => I16, true;
        i32 => I32, true;
        i64 => I64, true;
        u8 => U8, false;
        u16 => U16, false;
        u32 => U32, false;
        u64 => U64, false;
    }
    floats {
        f16 => F16;
        f32 => F32;
        f64 => F64;
    }
}
