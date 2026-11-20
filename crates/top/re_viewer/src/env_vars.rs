/// Enable allocation tracking (very slow - use it to find memory leaks!).
///
/// `RERUN_TRACK_ALLOCATIONS=1`
pub const RERUN_TRACK_ALLOCATIONS: &str = "RERUN_TRACK_ALLOCATIONS";

/// Install a hidden `rerun.desktop` entry and icon in `$XDG_DATA_HOME` (Linux only), so the dock shows the Rerun icon.
///
/// Enabled by default; `RERUN_DESKTOP_ENTRY=0` (or `false`, `no`, `off`) disables it.
pub const RERUN_DESKTOP_ENTRY: &str = "RERUN_DESKTOP_ENTRY";
