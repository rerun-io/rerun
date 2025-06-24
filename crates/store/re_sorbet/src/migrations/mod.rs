//! These are the migrations that were introduced for each version.
//!
//! For example [`v0_24`] contains the migrations that are needed
//! to go from [`v0_23`] to [`v_24`].

pub mod v0_23;
pub mod v0_24;

// TODO(grtlr): Eventually, we should have a trait the abstracts over
// migrations so that they will be easier to manage. But let's follow
// the rule of three here.
