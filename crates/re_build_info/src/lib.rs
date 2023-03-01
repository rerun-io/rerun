//! Information about the build of a Rust crate.
//!
//! To use this you also need to call `re_build_build_info::export_env_vars()` from your build.rs.

mod buid_info;
mod crate_version;

pub use buid_info::BuildInfo;
pub use crate_version::CrateVersion;

/// Create a [`BuildInfo`] at compile-time using environment variables exported by
/// calling `re_build_build_info::export_env_vars()` from your build.rs.
#[macro_export]
macro_rules! build_info {
    () => {
        $crate::BuildInfo {
            crate_name: env!("CARGO_PKG_NAME"),
            version: $crate::CrateVersion::parse(env!("CARGO_PKG_VERSION")),
            git_hash: env!("RE_BUILD_GIT_HASH"),
            git_branch: env!("RE_BUILD_GIT_BRANCH"),
            target_triple: env!("RE_BUILD_TARGET_TRIPLE"),
            datetime: env!("RE_BUILD_DATETIME"),
        }
    };
}
