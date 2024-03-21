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

    pub fn short_git_hash(&self) -> &str {
        if self.git_hash.is_empty() {
            ""
        } else {
            &self.git_hash[..7]
        }
    }

    pub fn is_final(&self) -> bool {
        self.version.meta.is_none()
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

        if cfg!(debug_assertions) {
            write!(f, " (debug)")?;
        }

        Ok(())
    }
}

// ---

use crate::CrateVersion;

impl CrateVersion {
    /// Attempts to parse a [`CrateVersion`] from a [`BuildInfo`]'s string representation (`rerun --version`).
    ///
    /// Refer to `BuildInfo as std::fmt::Display>::fmt` to see what the string representation is
    /// expected to look like. Roughly:
    /// ```ignore
    /// <name> <semver> [<rust_info>] <target> <branch> <commit> <build_date>
    /// ```
    pub fn try_parse_from_build_info_string(s: impl AsRef<str>) -> Result<CrateVersion, String> {
        let s = s.as_ref();
        let parts = s.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            return Err(format!("{s:?} is not a valid BuildInfo string"));
        }
        CrateVersion::try_parse(parts[1]).map_err(ToOwned::to_owned)
    }
}

#[test]
fn crate_version_from_build_info_string() {
    let build_info = BuildInfo {
        crate_name: "re_build_info",
        version: CrateVersion {
            major: 0,
            minor: 10,
            patch: 0,
            meta: Some(crate::crate_version::Meta::DevAlpha(7)),
        },
        rustc_version: "1.74.0 (d5c2e9c34 2023-09-13)",
        llvm_version: "16.0.5",
        git_hash: "",
        git_branch: "",
        is_in_rerun_workspace: true,
        target_triple: "x86_64-unknown-linux-gnu",
        datetime: "",
    };

    let build_info_str = build_info.to_string();

    {
        let expected_crate_version = build_info.version;
        let crate_version = CrateVersion::try_parse_from_build_info_string(build_info_str).unwrap();

        assert_eq!(expected_crate_version, crate_version);
    }
}
