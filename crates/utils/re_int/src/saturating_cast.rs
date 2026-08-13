/// Cast a number to another integer type, clamping to the target's range
/// instead of wrapping (`as`) or panicking (`TryFrom` + `unwrap`).
///
/// This restores the ergonomic call site of the old `saturating_cast` crate:
///
/// ```
/// use re_int::SaturatingCast as _;
///
/// assert_eq!(300_u32.saturating_cast::<u8>(), 255); // clamped to u8::MAX
/// assert_eq!((-5_i32).saturating_cast::<u8>(), 0); // clamped to u8::MIN
/// assert_eq!(u64::MAX.saturating_cast::<i64>(), i64::MAX);
/// assert_eq!(7_u32.saturating_cast::<i64>(), 7); // in range: unchanged
/// ```
pub trait SaturatingCast: Sized {
    /// Cast `self` to `Dst`, saturating at `Dst::MIN` / `Dst::MAX`.
    #[inline]
    fn saturating_cast<Dst>(self) -> Dst
    where
        Dst: SaturatingFrom<Self>,
    {
        Dst::saturating_from(self)
    }
}

// Implemented only for the concrete integer types (not a blanket `impl<T>`) so that
// calling `.saturating_cast()` on a reference (e.g. `&u64` from a by-ref destructure)
// auto-dereferences to the value type instead of failing to satisfy the bound.
macro_rules! impl_saturating_cast {
    ($($src:ty),* $(,)?) => {
        $( impl SaturatingCast for $src {} )*
    };
}

impl_saturating_cast!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

/// Build `Self` from `Src`, clamping to `Self`'s range instead of wrapping or panicking.
///
/// This is the saturating analog of [`From`]/[`TryFrom`].
/// Prefer calling it through [`SaturatingCast::saturating_cast`].
pub trait SaturatingFrom<Src> {
    /// Convert `src` to `Self`, saturating at `Self::MIN` / `Self::MAX`.
    fn saturating_from(src: Src) -> Self;
}

/// Implement [`SaturatingFrom`] for every source/target integer pair.
///
/// The direction of saturation is decided by the *source* signedness, which
/// makes each impl obviously correct and avoids the `absurd_extreme_comparisons`
/// lint (we never compare an unsigned value with `0`):
///
/// * unsigned source — [`TryFrom`] can only fail by overflowing, so saturate to `MAX`.
/// * signed source — fail low → `MIN`, fail high → `MAX`.
macro_rules! impl_saturating_from {
    (unsigned: $($src:ty),* $(,)?) => {
        $( impl_saturating_from!(@unsigned $src => i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize); )*
    };
    (signed: $($src:ty),* $(,)?) => {
        $( impl_saturating_from!(@signed $src => i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize); )*
    };
    (@unsigned $src:ty => $($dst:ty),*) => {
        $(
            impl SaturatingFrom<$src> for $dst {
                #[inline]
                fn saturating_from(src: $src) -> Self {
                    <$dst>::try_from(src).unwrap_or(<$dst>::MAX)
                }
            }
        )*
    };
    (@signed $src:ty => $($dst:ty),*) => {
        $(
            impl SaturatingFrom<$src> for $dst {
                #[inline]
                fn saturating_from(src: $src) -> Self {
                    <$dst>::try_from(src).unwrap_or(if src < 0 { <$dst>::MIN } else { <$dst>::MAX })
                }
            }
        )*
    };
}

impl_saturating_from!(unsigned: u8, u16, u32, u64, u128, usize);
impl_saturating_from!(signed: i8, i16, i32, i64, i128, isize);

#[cfg(test)]
mod tests {
    use super::SaturatingCast as _;

    #[test]
    fn widening_never_clamps() {
        assert_eq!(7_u32.saturating_cast::<i64>(), 7);
        assert_eq!(0_u8.saturating_cast::<u64>(), 0);
        assert_eq!(u8::MAX.saturating_cast::<u16>(), u16::from(u8::MAX));
        assert_eq!((-3_i8).saturating_cast::<i64>(), -3);
        assert_eq!(i32::MIN.saturating_cast::<i64>(), i64::from(i32::MIN));
    }

    #[test]
    fn same_type_is_identity() {
        assert_eq!(123_u64.saturating_cast::<u64>(), 123);
        assert_eq!((-123_i64).saturating_cast::<i64>(), -123);
        assert_eq!(u64::MAX.saturating_cast::<u64>(), u64::MAX);
    }

    #[test]
    fn unsigned_saturates_to_max() {
        assert_eq!(300_u32.saturating_cast::<u8>(), u8::MAX);
        assert_eq!(u64::MAX.saturating_cast::<i64>(), i64::MAX);
        assert_eq!(u64::MAX.saturating_cast::<i32>(), i32::MAX);
        assert_eq!(u128::MAX.saturating_cast::<u64>(), u64::MAX);
    }

    #[test]
    fn signed_saturates_both_ends() {
        // Narrowing overflow on the high end -> MAX.
        assert_eq!(1000_i32.saturating_cast::<i8>(), i8::MAX);
        assert_eq!(1000_i32.saturating_cast::<u8>(), u8::MAX);
        // Below the target's minimum -> MIN.
        assert_eq!((-1000_i32).saturating_cast::<i8>(), i8::MIN);
        // Negative into unsigned -> 0.
        assert_eq!((-1_i32).saturating_cast::<u8>(), 0);
        assert_eq!((-1_i64).saturating_cast::<u64>(), 0);
        assert_eq!(i128::MIN.saturating_cast::<u128>(), 0);
    }

    #[test]
    fn usize_isize() {
        assert_eq!(usize::MAX.saturating_cast::<i16>(), i16::MAX);
        assert_eq!(300_usize.saturating_cast::<u8>(), u8::MAX);
        assert_eq!(isize::MIN.saturating_cast::<i8>(), i8::MIN);
        assert_eq!(5_isize.saturating_cast::<u8>(), 5);
    }

    #[test]
    fn matches_call_site_conversions() {
        // re_mcap / re_auth / re_chunk_store / re_video: u64 -> i64.
        assert_eq!(u64::MAX.saturating_cast::<i64>(), i64::MAX);
        assert_eq!(0_u64.saturating_cast::<i64>(), 0);
        assert_eq!(42_u64.saturating_cast::<i64>(), 42);

        // re_view_spatial: usize -> i16 (DepthOffset).
        assert_eq!(10_usize.saturating_cast::<i16>(), 10);
        assert_eq!(usize::MAX.saturating_cast::<i16>(), i16::MAX);
    }

    #[test]
    fn works_through_a_reference() {
        // `re_mcap`'s `stats.rs` calls this on a `&u64` from a by-ref destructure.
        let value = u64::MAX;
        let by_ref: &u64 = &value;
        assert_eq!(by_ref.saturating_cast::<i64>(), i64::MAX);
    }
}
