/// Information about the build of a Rust crate.
///
/// To use this you also need to call `re_build_build_info::export_env_vars()` from your build.rs.
#[derive(Copy, Clone, Debug)]
pub struct BuildInfo {
    /// `CARGO_PKG_NAME`
    pub crate_name: &'static str,

    /// `CARGO_PKG_VERSION`
    pub version: &'static str,

    /// Git commit hash, or latest tag if not available.
    ///
    /// May have a `-dirty` suffix.
    pub git_hash: &'static str,

    /// Target architecture and OS
    ///
    /// Example: `xaarch64-apple-darwin`
    pub target_triple: &'static str,
}

/// Create a [`BuildInfo`] at compile-time using environment variables exported by
/// calling `re_build_build_info::export_env_vars()` from your build.rs.
#[macro_export]
macro_rules! build_info {
    () => {
        $crate::BuildInfo {
            crate_name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            git_hash: env!("RE_BUILD_GIT_HASH"),
            target_triple: env!("RE_BUILD_TARGET_TRIPLE"),
        }
    };
}
