//! Text logging (nothing to do with rerun logging) for use in rerun libraries.
//!
//! * `trace`: spammy things
//! * `debug`: things that might be useful when debugging
//! * `info`: things that we want to show to users
//! * `warn`: problems that we can recover from
//! * `error`: problems that lead to loss of functionality or data
//!
//! The `warn_once` etc macros are for when you want to suppress repeated
//! logging of the exact same message.

pub use tracing::{debug, error, info, trace, warn};

// The `re_log::info_once!(…)` etc are nice helpers, but the `log-once` crate is a bit lacking.
// In the future we should implement our own `tracing` layer and de-duplicate based on the callsite,
// similar to how the log console in a browser will automatically suppress duplicates.
pub use log_once::{debug_once, error_once, info_once, trace_once, warn_once};

mod setup;

pub use setup::*;
