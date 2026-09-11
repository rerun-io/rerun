//! Information about the build of a Rust crate.
//!
//! To use this you also need to call `re_build_tools::export_env_vars()` from your build.rs.

mod build_info;
mod crate_version;

pub use build_info::BuildInfo;
pub use crate_version::{CrateVersion, Meta};

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

/// The value [`exposed_version`] returns when the `EXPOSED_VERSION` environment variable is
/// not set (or empty).
///
/// Deliberately not a version: consumers that gate on a minimum server version must treat it
/// as "unknown", never as "old".
pub const EXPOSED_VERSION_UNSET: &str = "exposed-version-unset";

/// Returns the version string that servers expose on their version endpoints, metrics, and
/// analytics.
///
/// The version is injected at runtime through the `EXPOSED_VERSION` environment variable by
/// whoever deploys the binary (e.g. from the image tag); it is intentionally not baked into
/// the binary at compile time. When the variable is not set (or empty) — typically local dev
/// builds — this returns [`EXPOSED_VERSION_UNSET`].
///
/// The result is cached on first call.
pub fn exposed_version() -> &'static str {
    static EXPOSED_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    EXPOSED_VERSION.get_or_init(|| {
        std::env::var("EXPOSED_VERSION")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| EXPOSED_VERSION_UNSET.to_owned())
    })
}
