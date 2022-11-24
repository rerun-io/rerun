//! Environment variables we use.

/// Set an upper memory limit, e.g. `RERUN_MEMORY_LIMIT=16GB`.
pub const RERUN_MEMORY_LIMIT: &str = "RERUN_MEMORY_LIMIT";

/// Enable allocation tracking (very slow - use it to find memory leaks!).
///
/// `RERUN_TRACK_ALLOCATIONS=1`
pub const RERUN_TRACK_ALLOCATIONS: &str = "RERUN_TRACK_ALLOCATIONS";
