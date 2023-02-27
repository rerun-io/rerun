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
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            alpha: NO_ALPHA,
        }
    }

    pub const fn new_alpha(major: u8, minor: u8, patch: u8, alpha: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            alpha,
        }
    }

    pub const fn parse(s: &str) -> Self {
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
                i += 1;
            }
            assert!(num <= u8::MAX as u64);
            num as _
        }

        let s = s.as_bytes();

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
        while i < s.len() && s[i] != b'-' {
            i += 1;
        }
        let patch = parse_u8(s, patch_start, i);

        if i < s.len() {
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
            let alpha = parse_u8(s, i, s.len());
            Self::new_alpha(major, minor, patch, alpha)
        } else {
            Self::new(major, minor, patch)
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
    assert_eq!(parse("123.45.67"), RustVersion::new(123, 45, 67));
    assert_eq!(
        parse("123.45.67-alpha.89"),
        RustVersion::new_alpha(123, 45, 67, 89)
    );
}

#[test]
fn test_format_parse_roundtrip() {
    let parse = RustVersion::parse;
    for version in ["0.2.0", "1.2.3", "123.45.67", "123.45.67-alpha.89"] {
        assert_eq!(parse(version).to_string(), version);
    }
}
