//! Small numeric helper traits shared across the Rerun crates.
//!
//! * [`SaturatingCast`] — cast between integer types, clamping to the target's range.
//! * [`UnsignedAbs`] — absolute value of a signed integer without wrapping or panicking.

mod saturating_cast;
mod unsigned_abs;

pub use self::saturating_cast::{SaturatingCast, SaturatingFrom};
pub use self::unsigned_abs::UnsignedAbs;
