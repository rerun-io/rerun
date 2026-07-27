// TODO(rust-num/num-traits#315): waiting for https://github.com/rust-num/num-traits/issues/315 to land
/// Compute the absolute value of a signed integer without any wrapping or panicking.
pub trait UnsignedAbs {
    /// An unsigned type which is large enough to hold the absolute value of `Self`.
    type Unsigned;

    /// Computes the absolute value of `self` without any wrapping or panicking.
    fn unsigned_abs(self) -> Self::Unsigned;
}

macro_rules! impl_unsigned_abs {
    ($($signed:ty => $unsigned:ty),* $(,)?) => {
        $(
            impl UnsignedAbs for $signed {
                type Unsigned = $unsigned;

                #[inline]
                fn unsigned_abs(self) -> Self::Unsigned {
                    self.unsigned_abs()
                }
            }
        )*
    };
}

impl_unsigned_abs!(
    i8 => u8,
    i16 => u16,
    i32 => u32,
    i64 => u64,
    i128 => u128,
    isize => usize,
);
