//! Information about the build of a Rust crate.
//!
//! To use this you also need to call `re_build_tools::export_env_vars()` from your build.rs.

mod build_info;
mod crate_version;

pub use build_info::BuildInfo;
pub use crate_version::{CrateVersion, Meta};

// Re-export for use in macros
#[doc(hidden)]
pub use std::sync::OnceLock;

/// Create a [`BuildInfo`] at compile-time using environment variables exported by
/// calling `re_build_tools::export_env_vars()` from your build.rs.
#[macro_export]
macro_rules! build_info {
    () => {
        $crate::BuildInfo {
            crate_name: env!("CARGO_PKG_NAME").into(),
            features: env!("RE_BUILD_FEATURES").into(),
            version: $crate::CrateVersion::parse(env!("CARGO_PKG_VERSION")),
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            git_hash: env!("RE_BUILD_GIT_HASH").into(),
            git_branch: env!("RE_BUILD_GIT_BRANCH").into(),
            // TODO(cmc): `PartialEq` is not available in const contexts, so this won't actually
            // build if you try to instantiate a BuildInfo in a constant.
            is_in_rerun_workspace: env!("RE_BUILD_IS_IN_RERUN_WORKSPACE") == "yes",
            target_triple: env!("RE_BUILD_TARGET_TRIPLE").into(),
            datetime: env!("RE_BUILD_DATETIME").into(),
            is_debug_build: cfg!(debug_assertions),
        }
    };
}

/// Returns the exposed version string from the `EXPOSED_VERSION` environment variable.
///
/// If `EXPOSED_VERSION` is not set or empty, falls back to a version string constructed
/// from build info, prefixed with `"build:"`.
/// Format: `build:{CARGO_PKG_VERSION}[-{git_branch}][-{short_git_hash}]`
///
/// This macro must be called from a crate that has a build.rs calling
/// `re_build_tools::export_build_info_vars_for_crate()`.
///
/// The result is cached on first call.
#[macro_export]
macro_rules! exposed_version {
    () => {{
        static EXPOSED_VERSION: $crate::OnceLock<String> = $crate::OnceLock::new();

        EXPOSED_VERSION
            .get_or_init(|| {
                if let Some(version) = std::env::var("EXPOSED_VERSION")
                    .ok()
                    .filter(|v| !v.is_empty())
                {
                    version
                } else {
                    let info = $crate::build_info!();
                    let mut version = format!("build:{}", env!("CARGO_PKG_VERSION"));

                    if !info.git_branch.is_empty() {
                        version.push('-');
                        version.push_str(&info.git_branch);
                    }

                    if !info.short_git_hash().is_empty() {
                        version.push('-');
                        version.push_str(info.short_git_hash());
                    }

                    version
                }
            })
            .as_str()
    }};
}
