//! Small numeric helper traits shared across the Rerun crates.
//!
//! * [`SaturatingInto`] — cast between integer types, clamping to the target's range.
//! * [`UnsignedAbs`] — absolute value of a signed integer without wrapping or panicking.

mod saturating_into;
mod unsigned_abs;

pub use self::saturating_into::{SaturatingFrom, SaturatingInto};
pub use self::unsigned_abs::UnsignedAbs;
