/// We disallow version numbers larger than this in order to keep a few bits for future use.
///
/// If you are running up against this limit then feel free to bump it!
const MAX_NUM: u8 = 31;

const NO_ALPHA: u8 = 255;

/// Sub-set of semver supporting major, minor, patch and an optional `-alpha.X` suffix.
///
/// Examples: `1.2.3` and `1.2.3-alpha.4`.
///
/// This `struct` is designed to be space-efficient (32-bit)
/// so it can be used as a version string in file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RustVersion {
    major: u8,
    minor: u8,
    patch: u8,
    alpha: u8,
}

impl RustVersion {
    const fn new(major: u8, minor: u8, patch: u8) -> Self {
        assert!(
            major <= MAX_NUM && minor <= MAX_NUM && patch <= MAX_NUM,
            "Too large number in version string"
        );
        Self {
            major,
            minor,
            patch,
            alpha: NO_ALPHA,
        }
    }

    /// From a compact 32-bit representation crated with [`Self::to_bytes`].
    pub fn from_bytes([major, minor, patch, alpha]: [u8; 4]) -> Self {
        Self {
            major,
            minor,
            patch,
            alpha,
        }
    }

    /// A compact 32-bit representation. See also [`Self::from_bytes`].
    pub fn to_bytes(self) -> [u8; 4] {
        let Self {
            major,
            minor,
            patch,
            alpha,
        } = self;
        [major, minor, patch, alpha]
    }

    /// Is this an alpha-release?
    pub fn alpha(self) -> Option<u8> {
        (self.alpha != NO_ALPHA).then_some(self.alpha)
    }

    pub fn is_semver_compatible_with(self, other: RustVersion) -> bool {
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
            // `-alpha.X` suffix
            assert!(s[i] == b'-', "Expected `-alpha.X` suffix");
            i += 1;
            assert!(s[i] == b'a', "Expected `-alpha.X` suffix");
            i += 1;
            assert!(s[i] == b'l', "Expected `-alpha.X` suffix");
            i += 1;
            assert!(s[i] == b'p', "Expected `-alpha.X` suffix");
            i += 1;
            assert!(s[i] == b'h', "Expected `-alpha.X` suffix");
            i += 1;
            assert!(s[i] == b'a', "Expected `-alpha.X` suffix");
            i += 1;
            assert!(s[i] == b'.', "Expected `-alpha.X` suffix");
            i += 1;

            let alpha_start = i;
            while i < s.len() && s[i] != b'+' {
                i += 1;
            }
            parse_u8(s, alpha_start, i)
        } else {
            NO_ALPHA
        };

        // Any trailing `+22d293392.1` or similar?
        assert!(i == s.len(), "Rust version suffixes not supported");

        Self {
            major,
            minor,
            patch,
            alpha,
        }
    }
}

impl std::fmt::Display for RustVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            major,
            minor,
            patch,
            alpha,
        } = *self;
        if alpha == NO_ALPHA {
            write!(f, "{major}.{minor}.{patch}")
        } else {
            write!(f, "{major}.{minor}.{patch}-alpha.{alpha}")
        }
    }
}

#[test]
fn test_parse_version() {
    let parse = RustVersion::parse;
    assert_eq!(parse("0.2.0"), RustVersion::new(0, 2, 0));
    assert_eq!(parse("1.2.3"), RustVersion::new(1, 2, 3));
    assert_eq!(parse("12.23.24"), RustVersion::new(12, 23, 24));
    assert_eq!(
        parse("12.23.24-alpha.31"),
        RustVersion {
            major: 12,
            minor: 23,
            patch: 24,
            alpha: 31,
        }
    );
}

#[test]
fn test_format_parse_roundtrip() {
    let parse = RustVersion::parse;
    for version in ["0.2.0", "1.2.3", "12.23.24", "12.23.24-alpha.31"] {
        assert_eq!(parse(version).to_string(), version);
    }
}

#[test]
fn test_compatibility() {
    fn are_compatible(a: &str, b: &str) -> bool {
        RustVersion::parse(a).is_semver_compatible_with(RustVersion::parse(b))
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
