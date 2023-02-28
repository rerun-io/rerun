/// Information about the build of a Rust crate.
///
/// Create this with [`crate::build_info`].
///
/// The `git_` fields are all empty on failure. Most likely git fails because we're not in a git repository
/// to begin with, which happens because we've imported the published crate from crates.io.
///
/// There are a few other cases though, like
/// - `git` is not installed
/// - the user downloaded rerun as a tarball and then imported via a `path = ...` import
/// - others?
#[derive(Copy, Clone, Debug)]
pub struct BuildInfo {
    /// `CARGO_PKG_NAME`
    pub crate_name: &'static str,

    /// Parsed `CARGO_PKG_VERSION`
    pub version: super::RustVersion,

    /// Git commit hash, or empty string.
    pub git_hash: &'static str,

    /// Current git branch, or empty string.
    pub git_branch: &'static str,

    /// Target architecture and OS
    ///
    /// Example: `xaarch64-apple-darwin`
    pub target_triple: &'static str,

    /// ISO 8601 / RFC 3339 build time.
    ///
    /// Example: `"2023-02-23T19:33:26Z"`
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
            git_hash,
            git_branch,
            target_triple,
            datetime,
        } = self;

        write!(f, "{crate_name} {version}")?;

        write!(f, " {target_triple}")?;

        if !git_branch.is_empty() {
            write!(f, " {git_branch}")?;
        }
        if !git_hash.is_empty() {
            let git_hash: String = git_hash.chars().take(7).collect(); // shorten
            write!(f, " {git_hash}")?;
        }

        write!(f, ", built {datetime}")?;

        Ok(())
    }
}
