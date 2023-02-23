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

    /// ISO 8601 / RFC 3339 build time.
    ///
    /// Example: `"2023-02-23T19:33:26Z"`
    pub datetime: &'static str,
}

/// For use with e.g. `--version`
impl std::fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            crate_name,
            version,
            git_hash,
            target_triple,
            datetime,
        } = self;

        if git_hash.is_empty() || version == git_hash {
            // This happens when you don't build in a git repository, i.e. on users machines.
            write!(
                f,
                "{crate_name} {version} {target_triple}, built {datetime}"
            )
        } else {
            write!(
                f,
                "{crate_name} {version} {git_hash} {target_triple}, built {datetime}"
            )
        }
    }
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
            datetime: env!("RE_BUILD_DATETIME"),
        }
    };
}
