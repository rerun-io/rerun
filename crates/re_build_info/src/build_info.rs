/// Information about the build of a Rust crate.
///
/// Create this with [`crate::build_info!`].
///
/// The `git_` fields are all empty on failure. Most likely git fails because we're not in a git repository
/// to begin with, which happens because we've imported the published crate from crates.io.
///
/// There are a few other cases though, like
/// - `git` is not installed
/// - the user downloaded rerun as a tarball and then imported via a `path = …` import
/// - others?
#[derive(Copy, Clone, Debug)]
pub struct BuildInfo {
    /// `CARGO_PKG_NAME`
    pub crate_name: &'static str,

    /// Crate version, parsed from `CARGO_PKG_VERSION`, ignoring any `+metadata` suffix.
    pub version: super::CrateVersion,

    /// The raw version string of the Rust compiler used, or an empty string.
    pub rustc_version: &'static str,

    /// The raw version string of the LLVM toolchain used, or an empty string.
    pub llvm_version: &'static str,

    /// Git commit hash, or empty string.
    pub git_hash: &'static str,

    /// Current git branch, or empty string.
    pub git_branch: &'static str,

    /// True if we are building within the rerun repository workspace.
    ///
    /// This is a good proxy for "user checked out the project and built it from source".
    pub is_in_rerun_workspace: bool,

    /// Target architecture and OS
    ///
    /// Example: `xaarch64-apple-darwin`
    pub target_triple: &'static str,

    /// ISO 8601 / RFC 3339 build time.
    ///
    /// Example: `"2023-02-23T19:33:26Z"`
    ///
    /// Empty if unknown.
    pub datetime: &'static str,
}

impl BuildInfo {
    pub fn git_hash_or_tag(&self) -> String {
        if self.git_hash.is_empty() {
            format!("v{}", self.version)
        } else {
            self.git_hash.to_owned()
        }
    }
}

/// For use with e.g. `--version`
impl std::fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            crate_name,
            version,
            rustc_version,
            llvm_version,
            git_hash,
            git_branch,
            is_in_rerun_workspace: _,
            target_triple,
            datetime,
        } = self;

        let rustc_version = (!rustc_version.is_empty()).then(|| format!("rustc {rustc_version}"));
        let llvm_version = (!llvm_version.is_empty()).then(|| format!("LLVM {llvm_version}"));

        write!(f, "{crate_name} {version}")?;

        if let Some(rustc_version) = rustc_version {
            write!(f, " [{rustc_version}")?;
            if let Some(llvm_version) = llvm_version {
                write!(f, ", {llvm_version}")?;
            }
            write!(f, "]")?;
        }

        if !target_triple.is_empty() {
            write!(f, " {target_triple}")?;
        }

        if !git_branch.is_empty() {
            write!(f, " {git_branch}")?;
        }
        if !git_hash.is_empty() {
            let git_hash: String = git_hash.chars().take(7).collect(); // shorten
            write!(f, " {git_hash}")?;
        }

        if !datetime.is_empty() {
            write!(f, ", built {datetime}")?;
        }

        Ok(())
    }
}
