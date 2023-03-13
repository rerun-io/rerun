/// Enable allocation tracking (very slow - use it to find memory leaks!).
///
/// `RERUN_TRACK_ALLOCATIONS=1`
pub const RERUN_TRACK_ALLOCATIONS: &str = "RERUN_TRACK_ALLOCATIONS";

/// Set an upper limit on how much memory the Rerun Viewer should use, e.g. e.g. "10MiB" or
/// "2GiB", above which it'll start dropping old data (according to the order it was received in).
///
/// `RERUN_MEMORY_LIMIT=4GiB`
pub const RERUN_MEMORY_LIMIT: &str = "RERUN_MEMORY_LIMIT";

/// Set a maximum input latency, e.g. "200ms" or "10s", above which the Rerun server will start
/// dropping packets.
pub const RERUN_LATENCY_LIMIT: &str = "RERUN_LATENCY_LIMIT";
