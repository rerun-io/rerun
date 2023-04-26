/// We disallow version numbers larger than this in order to keep a few bits for future use.
///
/// If you are running up against this limit then feel free to bump it!
const MAX_NUM: u8 = 31;

const IS_ALPHA_BIT: u8 = 1 << 7;
const IS_PRERELEASE_BIT: u8 = 1 << 6;

/// The version of a Rerun crate.
///
/// Sub-set of semver supporting `major.minor.patch` plus an optional `-alpha.X`.
///
/// When parsing, any `+metadata` suffix is ignored.
///
/// Examples: `1.2.3`, `1.2.3-alpha.4`.
///
/// We use `-alpha.X` when we publish pre-releases to crates.io and PyPI.
///
/// We use a `+githash` suffix for continuous pre-releases that you can download from our GitHub.
/// We do NOT store that in this struct. See also `scripts/version_util.py`.
///
/// The version numbers aren't allowed to be very large (current max: 31).
/// This limited subset it chosen so that we can encode the version in 32 bits
/// in our `.rrd` files and on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrateVersion {
    major: u8,
    minor: u8,
    patch: u8,
    alpha: Option<u8>,
    prerelease: bool,
}

impl CrateVersion {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        assert!(
            major <= MAX_NUM && minor <= MAX_NUM && patch <= MAX_NUM,
            "Too large number in version string"
        );
        Self {
            major,
            minor,
            patch,
            alpha: None,
            prerelease: false,
        }
    }

    /// Whether or not this build is a prerelease (a version ending with +commit suffix)
    pub fn is_prerelease(&self) -> bool {
        self.prerelease
    }

    /// From a compact 32-bit representation crated with [`Self::to_bytes`].
    pub fn from_bytes([major, minor, patch, suffix_byte]: [u8; 4]) -> Self {
        let is_alpha = (suffix_byte & IS_ALPHA_BIT) != 0;
        let is_prerelease = (suffix_byte & IS_PRERELEASE_BIT) != 0;
        let alpha_version = suffix_byte & !(IS_ALPHA_BIT | IS_PRERELEASE_BIT);

        Self {
            major,
            minor,
            patch,
            alpha: is_alpha.then_some(alpha_version),
            prerelease: is_prerelease,
        }
    }

    /// A compact 32-bit representation. See also [`Self::from_bytes`].
    pub fn to_bytes(self) -> [u8; 4] {
        let Self {
            major,
            minor,
            patch,
            alpha,
            prerelease,
        } = self;

        let mut suffix_byte = if let Some(alpha) = alpha {
            IS_ALPHA_BIT | alpha
        } else {
            0
        };

        suffix_byte |= if prerelease { IS_PRERELEASE_BIT } else { 0 };

        [major, minor, patch, suffix_byte]
    }

    pub fn is_compatible_with(self, other: CrateVersion) -> bool {
        if self.alpha != other.alpha {
            return false; // Alphas can contain breaking changes
        }

        if self.major == 0 {
            // before 1.0.0 we break compatibility using the minor:
            (self.major, self.minor) == (other.major, other.minor)
        } else {
            // major version is the only breaking change:
            self.major == other.major
        }
    }

    /// Parse a semver version string, ignoring any trailing `+metadata`.
    pub const fn parse(version_string: &str) -> Self {
        // Note that this is a const function, which means we are extremely limited in what we can do!

        const fn parse_u8(s: &[u8], begin: usize, end: usize) -> u8 {
            assert!(begin < end);
            assert!(
                s[begin] != b'0' || begin + 1 == end,
                "multi-digit number cannot start with zero"
            );

            let mut num = 0u64;
            let mut i = begin;

            while i < end {
                let c = s[i];
                assert!(
                    b'0' <= c && c <= b'9',
                    "Unexpected non-digit in version string"
                );
                let digit = c - b'0';
                num = num * 10 + digit as u64;
                assert!(num <= MAX_NUM as _, "Too large number in rust version");
                i += 1;
            }
            assert!(num <= u8::MAX as u64);
            num as _
        }

        let s = version_string.as_bytes();

        let mut i = 0;
        while s[i] != b'.' {
            i += 1;
        }
        let major = parse_u8(s, 0, i);

        i += 1;
        let minor_start = i;
        while s[i] != b'.' {
            i += 1;
        }
        let minor = parse_u8(s, minor_start, i);

        i += 1;
        let patch_start = i;
        while i < s.len() && s[i] != b'-' && s[i] != b'+' {
            i += 1;
        }
        let patch = parse_u8(s, patch_start, i);

        if i == s.len() {
            return Self::new(major, minor, patch);
        }

        let alpha = if s[i] == b'-' {
            // `-alpha.X` suffix (Called "pre-release version" in semver).
            // Comparing strings in `const` functions is fun:
            assert!(
                s[i] == b'-'
                    && s[i + 1] == b'a'
                    && s[i + 2] == b'l'
                    && s[i + 3] == b'p'
                    && s[i + 4] == b'h'
                    && s[i + 5] == b'a'
                    && s[i + 6] == b'.',
                "Expected `-alpha.X` suffix"
            );
            i += 7;

            let alpha_start = i;
            while i < s.len() && s[i] != b'+' {
                i += 1;
            }
            Some(parse_u8(s, alpha_start, i))
        } else {
            None
        };

        // If there are additional characters past alpha, it must be a prerelease
        let prerelease = if i < s.len() {
            assert!(s[i] == b'+', "Unexpected suffix");
            true
        } else {
            false
        };

        Self {
            major,
            minor,
            patch,
            alpha,
            prerelease,
        }
    }
}

impl std::fmt::Display for CrateVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            major,
            minor,
            patch,
            alpha,
            prerelease,
        } = *self;

        write!(f, "{major}.{minor}.{patch}")?;
        if let Some(alpha) = alpha {
            write!(f, "-alpha.{alpha}")?;
        }
        if prerelease {
            write!(f, "+")?;
        }
        Ok(())
    }
}

#[test]
fn test_parse_version() {
    let parse = CrateVersion::parse;
    assert_eq!(parse("0.2.0"), CrateVersion::new(0, 2, 0));
    assert_eq!(parse("1.2.3"), CrateVersion::new(1, 2, 3));
    assert_eq!(parse("12.23.24"), CrateVersion::new(12, 23, 24));
    assert_eq!(
        parse("12.23.24-alpha.31"),
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            alpha: Some(31),
            prerelease: false
        }
    );
    assert_eq!(
        parse("12.23.24+foo"),
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            alpha: None,
            prerelease: true
        }
    );
    assert_eq!(
        parse("12.23.24-alpha.31+bar"),
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            alpha: Some(31),
            prerelease: true
        }
    );
}

#[test]
fn test_format_parse_roundtrip() {
    let parse = CrateVersion::parse;
    for version in [
        "0.2.0",
        "1.2.3",
        "12.23.24",
        "12.23.24-alpha.31",
        // These do NOT round-trip, because we ignore the `+metadata`:
        // "12.23.24+githash",
        // "12.23.24-alpha.31+foobar",
    ] {
        assert_eq!(parse(version).to_string(), version);
    }
}

#[test]
fn test_format_parse_roundtrip_bytes() {
    let parse = CrateVersion::parse;
    for version in [
        "0.2.0",
        "1.2.3",
        "12.23.24",
        "12.23.24-alpha.31",
        "12.23.24-alpha.31+foo",
    ] {
        let version = parse(version);
        let bytes = version.to_bytes();
        assert_eq!(CrateVersion::from_bytes(bytes), version);
    }
}

#[test]
fn test_compatibility() {
    fn are_compatible(a: &str, b: &str) -> bool {
        CrateVersion::parse(a).is_compatible_with(CrateVersion::parse(b))
    }

    assert!(are_compatible("0.2.0", "0.2.0"));
    assert!(are_compatible("0.2.0", "0.2.1"));
    assert!(are_compatible("1.2.0", "1.3.0"));
    assert!(!are_compatible("0.2.0", "1.2.0"));
    assert!(!are_compatible("0.2.0", "0.3.0"));
    assert!(are_compatible("0.2.0-alpha.0", "0.2.0-alpha.0"));
    assert!(
        !are_compatible("0.2.0-alpha.0", "0.2.0-alpha.1"),
        "Alphas are always incompatible"
    );
}
