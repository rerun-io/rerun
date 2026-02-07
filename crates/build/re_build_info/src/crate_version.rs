mod meta {
    pub const TAG_MASK: u8 = 0b11000000;
    pub const VALUE_MASK: u8 = 0b00111111;
    pub const MAX_VALUE: u8 = VALUE_MASK;

    pub const RC: u8 = 0b01000000;
    pub const ALPHA: u8 = 0b10000000;
    pub const DEV_ALPHA: u8 = 0b11000000;
}

/// The version of a Rerun crate.
///
/// Sub-set of semver supporting `major.minor.patch-{alpha,rc}.N+dev`.
///
/// The string value of build metadata is not preserved.
///
/// Examples: `1.2.3`, `1.2.3-alpha.4`, `1.2.3-alpha.1+dev`.
///
/// `-alpha.N+dev` versions are used for local or CI builds.
/// `-alpha.N` versions are used for weekly releases.
/// `-rc.N` versions are used for release candidates as we're preparing for a full release.
///
/// The version numbers (`N`) aren't allowed to be very large (current max: 63).
/// This limited subset is chosen so that we can encode the version in 32 bits
/// in our `.rrd` files and on the wire.
///
/// Here is the current binary format:
/// ```text,ignore
/// major    minor    patch    meta
/// 00000000 00000000 00000000 00NNNNNN
///                            ▲▲▲    ▲
///                            ││└─┬──┘
///                            ││  └─ N
///                            │└─ rc/dev
///                            └─ alpha
/// ```
///
/// The valid bit patterns for `meta` are:
/// - `10NNNNNN` -> `-alpha.N`
/// - `11NNNNNN` -> `-alpha.N+dev`
/// - `01NNNNNN` -> `-rc.N`
/// - `00000000` -> none of the above
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrateVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub meta: Option<Meta>,
}

impl Ord for CrateVersion {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            major,
            minor,
            patch,
            meta: _,
        } = self;

        match major.cmp(&other.major) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match minor.cmp(&other.minor) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        patch.cmp(&other.patch)
    }
}

impl PartialOrd for CrateVersion {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl CrateVersion {
    pub const LOCAL: Self = Self::parse(env!("CARGO_PKG_VERSION"));

    /// If this version is stable returns it, otherwise returns the version prior to that.
    ///
    /// Doesn't have knowledge of release patched versions, so the returned version will be conservative,
    /// and not contain any patched versions.
    /// Similarly, it doesn't know whether a version release was skipped.
    /// ```
    /// # use re_build_info::CrateVersion;
    /// assert_eq!(CrateVersion::parse("0.19.1").latest_stable(), CrateVersion::parse("0.19.1"));
    /// assert_eq!(CrateVersion::parse("0.19.1-rc.1").latest_stable(), CrateVersion::parse("0.19.1-rc.1"));
    /// assert_eq!(CrateVersion::parse("0.19.1-alpha.1+dev").latest_stable(), CrateVersion::parse("0.19.0"));
    /// assert_eq!(CrateVersion::parse("0.19.0-alpha.1+dev").latest_stable(), CrateVersion::parse("0.18.0"));
    /// assert_eq!(CrateVersion::parse("2.0.0-alpha.1+dev").latest_stable(), CrateVersion::parse("1.0.0"));
    /// ```
    pub fn latest_stable(self) -> Self {
        // If it is a dev version, walk one version back.
        if self.is_dev() {
            if self.patch == 0 {
                // There might be a patched version of the latest minor/major, but we don't know that unfortunately.
                if self.minor == 0 {
                    Self {
                        major: self.major - 1,
                        minor: 0,
                        patch: 0,
                        meta: None,
                    }
                } else {
                    Self {
                        major: self.major,
                        minor: self.minor - 1,
                        patch: 0,
                        meta: None,
                    }
                }
            } else {
                Self {
                    major: self.major,
                    minor: self.minor,
                    patch: self.patch - 1,
                    meta: None,
                }
            }
        } else {
            self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Meta {
    Rc(u8),
    Alpha(u8),

    /// `0.19.1-alpha.2+dev` or `0.19.1-alpha.2+aab0b4e`
    DevAlpha {
        alpha: u8,

        /// The commit hash, if known
        ///
        /// `None` corresponds to `+dev`.
        ///
        /// In order to support compile-time parsing of versions strings
        /// this needs to be `&'static` and `[u8]` instead of `String`.
        /// But in practice this is guaranteed to be valid UTF-8.
        ///
        /// The commit hash is NOT sent over the wire,
        /// so `0.19.1-alpha.2+aab0b4e` will end up as `0.19.1-alpha.2+dev`
        /// on the other end.
        commit: Option<&'static [u8]>,
    },
}

impl Meta {
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Rc(value) => value | meta::RC,
            Self::Alpha(value) => value | meta::ALPHA,

            // We ignore the commit hash, if any
            Self::DevAlpha { alpha, .. } => alpha | meta::DEV_ALPHA,
        }
    }

    pub const fn from_byte(v: u8) -> Option<Self> {
        let tag = v & meta::TAG_MASK;
        let value = v & meta::VALUE_MASK;
        match tag {
            meta::RC => Some(Self::Rc(value)),
            meta::ALPHA => Some(Self::Alpha(value)),
            meta::DEV_ALPHA => Some(Self::DevAlpha {
                alpha: value,
                commit: None,
            }),
            _ => None,
        }
    }
}

/// Helper function to slice slices in a `const` context.
/// Instead of using this directly, use the `slice` macro.
///
/// This is equivalent to `v[start..end]`.
const fn const_u8_slice_util(v: &[u8], start: Option<usize>, end: Option<usize>) -> &[u8] {
    let (start, end) = match (start, end) {
        (Some(start), Some(end)) => (start, end),
        (Some(start), None) => (start, v.len()),
        (None, Some(end)) => (0, end),
        (None, None) => return v,
    };

    assert!(start <= v.len());
    assert!(end <= v.len());
    assert!(start <= end);

    {
        // The only reason we do this is to allow slicing in `const` functions.
        #![expect(unsafe_code)]

        let ptr = v.as_ptr();
        // SAFETY:
        // - the read is valid, because the following is true:
        //   - `ptr` is valid for reads of `len` elements, because it is taken from a valid slice.
        //     this means it is already guaranteed to be non-null and properly aligned, and the
        //     entire length of the slice is contained within a single allocated object.
        //   - `start <= len && end <= len && start <= end`
        // - the returned slice appears to be a shared borrow from `v`,
        //   so the borrow checker will ensure users will not mutate `v`
        //   until this slice is dropped.
        unsafe { std::slice::from_raw_parts(ptr.add(start), end - start) }
    }
}

/// Slice `s` by some `start` and `end` bounds.
///
/// This is equivalent to doing `s[start..end]`,
/// but works in a `const` context.
macro_rules! slice {
    ($s:expr, .., $end:expr) => {
        const_u8_slice_util($s, None, Some($end))
    };
    ($s:expr, $start:expr, ..) => {
        const_u8_slice_util($s, Some($start), None)
    };
    ($s:expr, $start:expr, $end:expr) => {
        const_u8_slice_util($s, Some($start), Some($end))
    };
}

const fn equals(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let len = a.len();
    let mut i = 0;
    while i < len {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn split_at(s: &[u8], i: usize) -> (&[u8], &[u8]) {
    (slice!(s, .., i), slice!(s, i, ..))
}

impl CrateVersion {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            meta: None,
        }
    }

    /// True is this version has no metadata at all (rc, dev, alpha, etc).
    ///
    /// I.e. it's an actual, final release.
    pub fn is_release(&self) -> bool {
        self.meta.is_none()
    }

    /// Whether or not this build has a `+dev` suffix.
    ///
    /// This is used to identify builds which are not explicit releases,
    /// such as local builds and CI builds for every commit.
    pub fn is_dev(&self) -> bool {
        matches!(self.meta, Some(Meta::DevAlpha { .. }))
    }

    /// Whether or not this is an alpha version (`-alpha.N` or `-alpha.N+dev`).
    pub fn is_alpha(&self) -> bool {
        matches!(self.meta, Some(Meta::Alpha(..) | Meta::DevAlpha { .. }))
    }

    /// Whether or not this is a release candidate (`-rc.N`).
    pub fn is_rc(&self) -> bool {
        matches!(self.meta, Some(Meta::Rc(..)))
    }

    /// From a compact 32-bit representation crated with [`Self::to_bytes`].
    pub fn from_bytes([major, minor, patch, meta]: [u8; 4]) -> Self {
        Self {
            major,
            minor,
            patch,
            meta: Meta::from_byte(meta),
        }
    }

    /// A compact 32-bit representation. See also [`Self::from_bytes`].
    pub fn to_bytes(self) -> [u8; 4] {
        [
            self.major,
            self.minor,
            self.patch,
            self.meta.map(Meta::to_byte).unwrap_or_default(),
        ]
    }

    #[expect(clippy::unnested_or_patterns)]
    pub fn is_compatible_with(self, other: Self) -> bool {
        match (self.meta, other.meta) {
            // release candidates are always compatible with each other
            // and their finalized version:
            //   1.0.0-rc.1 == 1.0.0-rc.2 == 1.0.0
            (Some(Meta::Rc(..)), Some(Meta::Rc(..)))
            | (Some(Meta::Rc(..)), None)
            | (None, Some(Meta::Rc(..))) => {}
            (this, other) => {
                if this != other {
                    // Alphas can contain breaking changes
                    return false;
                }
            }
        }

        if self.major == 0 {
            // before 1.0.0 we break compatibility using the minor:
            (self.major, self.minor) == (other.major, other.minor)
        } else {
            // major version is the only breaking change:
            self.major == other.major
        }
    }
}

impl CrateVersion {
    /// Parse a version string according to our subset of semver.
    ///
    /// See [`CrateVersion`] for more information.
    pub const fn parse(version_string: &'static str) -> Self {
        match Self::try_parse(version_string) {
            Ok(version) => version,
            Err(_err) => {
                // We previously used const_panic to concatenate the actual version but it crashed
                // the 1.72.0 linker on mac :/
                panic!("invalid version string")
            }
        }
    }

    /// Parse a version string according to our subset of semver.
    ///
    /// See [`CrateVersion`] for more information.
    pub const fn try_parse(version_string: &'static str) -> Result<Self, &'static str> {
        // Note that this is a const function, which means we are extremely limited in what we can do!

        const fn maybe(s: &[u8], c: u8) -> (bool, &[u8]) {
            if !s.is_empty() && s[0] == c {
                (true, slice!(s, 1, ..))
            } else {
                (false, s)
            }
        }

        const fn maybe_token<'a>(s: &'a [u8], token: &[u8]) -> (bool, &'a [u8]) {
            if s.len() < token.len() {
                return (false, s);
            }

            let (left, right) = split_at(s, token.len());
            if equals(left, token) {
                (true, right)
            } else {
                (false, s)
            }
        }

        macro_rules! eat {
            ($s:ident, $c:expr, $msg:literal) => {{
                if $s.is_empty() || $s[0] != $c {
                    return Err($msg);
                }
                slice!($s, 1, ..)
            }};
        }

        macro_rules! eat_u8 {
            ($s:ident, $msg:literal) => {{
                if $s.is_empty() {
                    return Err($msg);
                }

                if $s.len() > 1 && $s[1].is_ascii_digit() {
                    if $s[0] == b'0' {
                        return Err("multi-digit number cannot start with zero");
                    }
                }

                let mut num = 0u64;
                let mut i = 0;
                while i < $s.len() && $s[i].is_ascii_digit() {
                    let digit = ($s[i] - b'0') as u64;
                    num = num * 10 + digit;
                    i += 1;
                }

                if num > u8::MAX as u64 {
                    return Err("digit cannot be larger than 255");
                }
                let num = num as u8;
                let remainder = slice!($s, i, ..);

                (num, remainder)
            }};
        }

        let mut s = version_string.as_bytes();
        let (major, minor, patch);
        let mut meta = None;

        (major, s) = eat_u8!(s, "expected major version number");
        s = eat!(s, b'.', "expected `.` after major version number");
        (minor, s) = eat_u8!(s, "expected minor version number");
        s = eat!(s, b'.', "expected `.` after minor version number");
        (patch, s) = eat_u8!(s, "expected patch version number");

        if let (true, remainder) = maybe(s, b'-') {
            s = remainder;

            let build;
            if let (true, remainder) = maybe_token(s, b"alpha") {
                s = eat!(remainder, b'.', "expected `.` after `-alpha`");
                (build, s) = eat_u8!(s, "expected digit after `-alpha.`");
                if build > meta::MAX_VALUE {
                    return Err("`-alpha` build number is larger than 63");
                }
                meta = Some(Meta::Alpha(build));
            } else if let (true, remainder) = maybe_token(s, b"rc") {
                s = eat!(remainder, b'.', "expected `.` after `-rc`");
                (build, s) = eat_u8!(s, "expected digit after `-rc.`");
                if build > meta::MAX_VALUE {
                    return Err("`-rc` build number is larger than 63");
                }
                meta = Some(Meta::Rc(build));
            } else {
                return Err("expected `alpha` or `rc` after `-`");
            }
        }

        if let (true, remainder) = maybe(s, b'+') {
            s = remainder;
            match meta {
                Some(Meta::Alpha(build)) => {
                    if let (true, remainder) = maybe_token(s, b"dev") {
                        s = remainder;
                        meta = Some(Meta::DevAlpha {
                            alpha: build,
                            commit: None,
                        });
                    } else if s.is_empty() {
                        return Err("expected `dev` after `+`");
                    } else {
                        let commit_hash = s;
                        s = &[];
                        meta = Some(Meta::DevAlpha {
                            alpha: build,
                            commit: Some(commit_hash),
                        });
                    }
                }
                Some(..) => return Err("unexpected `-rc` with `+dev`"),
                None => return Err("unexpected `+dev` without `-alpha`"),
            }
        }

        if !s.is_empty() {
            return Err("expected end of string");
        }

        Ok(Self {
            major,
            minor,
            patch,
            meta,
        })
    }
}

impl std::fmt::Display for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rc(build) => write!(f, "-rc.{build}"),
            Self::Alpha(build) => write!(f, "-alpha.{build}"),
            Self::DevAlpha { alpha, commit } => {
                if let Some(commit) = commit.and_then(|s| std::str::from_utf8(s).ok()) {
                    write!(f, "-alpha.{alpha}+{commit}")
                } else {
                    write!(f, "-alpha.{alpha}+dev")
                }
            }
        }
    }
}

impl std::fmt::Display for CrateVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            major,
            minor,
            patch,
            meta,
        } = *self;

        write!(f, "{major}.{minor}.{patch}")?;
        if let Some(meta) = meta {
            write!(f, "{meta}")?;
        }
        Ok(())
    }
}

impl re_byte_size::SizeBytes for CrateVersion {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

#[test]
fn test_parse_version() {
    macro_rules! assert_parse_ok {
        ($input:literal, $expected:expr) => {
            assert_eq!(CrateVersion::try_parse($input), Ok($expected))
        };
    }

    assert_parse_ok!("0.2.0", CrateVersion::new(0, 2, 0));
    assert_parse_ok!("0.2.0", CrateVersion::new(0, 2, 0));
    assert_parse_ok!("1.2.3", CrateVersion::new(1, 2, 3));
    assert_parse_ok!("12.23.24", CrateVersion::new(12, 23, 24));
    assert_parse_ok!(
        "12.23.24-rc.63",
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            meta: Some(Meta::Rc(63)),
        }
    );
    assert_parse_ok!(
        "12.23.24-alpha.63",
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            meta: Some(Meta::Alpha(63)),
        }
    );
    assert_parse_ok!(
        "12.23.24-alpha.63+dev",
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            meta: Some(Meta::DevAlpha {
                alpha: 63,
                commit: None
            }),
        }
    );
    // We use commit hash suffixes in some cases:
    assert_parse_ok!(
        "12.23.24-alpha.63+aab0b4e",
        CrateVersion {
            major: 12,
            minor: 23,
            patch: 24,
            meta: Some(Meta::DevAlpha {
                alpha: 63,
                commit: Some(b"aab0b4e")
            }),
        }
    );
}

#[test]
fn test_format_parse_roundtrip() {
    for version in [
        "0.2.0",
        "1.2.3",
        "12.23.24",
        "12.23.24-rc.63",
        "12.23.24-alpha.63",
        "12.23.24-alpha.63+dev",
        "12.23.24-alpha.63+aab0b4e",
    ] {
        assert_eq!(CrateVersion::parse(version).to_string(), version);
    }
}

#[test]
fn test_format_parse_roundtrip_bytes() {
    for version in [
        "0.2.0",
        "1.2.3",
        "12.23.24",
        "12.23.24-rc.63",
        "12.23.24-alpha.63",
        "12.23.24-alpha.63+dev",
        // "12.23.24-alpha.63+aab0b4e", // we don't serialize commit hashes!
    ] {
        let version = CrateVersion::parse(version);
        let bytes = version.to_bytes();
        assert_eq!(CrateVersion::from_bytes(bytes), version);
    }
}

#[test]
fn test_compatibility() {
    fn are_compatible(a: &'static str, b: &'static str) -> bool {
        CrateVersion::parse(a).is_compatible_with(CrateVersion::parse(b))
    }

    assert!(are_compatible("0.2.0", "0.2.0"));
    assert!(are_compatible("0.2.0", "0.2.1"));
    assert!(are_compatible("1.2.0", "1.3.0"));
    assert!(
        !are_compatible("0.2.0", "1.2.0"),
        "Different major versions are incompatible"
    );
    assert!(
        !are_compatible("0.2.0", "0.3.0"),
        "Different minor versions are incompatible"
    );
    assert!(are_compatible("0.2.0-alpha.0", "0.2.0-alpha.0"));
    assert!(are_compatible("0.2.0-rc.0", "0.2.0-rc.0"));
    assert!(
        !are_compatible("0.2.0-rc.0", "0.2.0-alpha.0"),
        "Rc and Alpha are incompatible"
    );
    assert!(
        !are_compatible("0.2.0-rc.0", "0.2.0-alpha.0+dev"),
        "Rc and Dev are incompatible"
    );
    assert!(
        !are_compatible("0.2.0-alpha.0", "0.2.0-alpha.0+dev"),
        "Alpha and Dev are incompatible"
    );
    assert!(
        !are_compatible("0.2.0-alpha.0", "0.2.0-alpha.1"),
        "Different alpha builds are always incompatible"
    );
    assert!(
        are_compatible("0.2.0-rc.0", "0.2.0-rc.1"),
        "Different rc builds are always compatible"
    );
    assert!(
        are_compatible("0.2.0-rc.0", "0.2.0"),
        "rc build is compatible with the finalized version"
    );
    assert!(
        are_compatible("0.2.0", "0.2.1-rc.0"),
        "rc build is compatible by patch version"
    );
}

#[test]
fn test_bad_parse() {
    macro_rules! assert_parse_err {
        ($input:literal, $expected:literal) => {
            assert_eq!(CrateVersion::try_parse($input), Err($expected))
        };
    }

    assert_parse_err!("10", "expected `.` after major version number");
    assert_parse_err!("10.", "expected minor version number");
    assert_parse_err!("10.0", "expected `.` after minor version number");
    assert_parse_err!("10.0.", "expected patch version number");
    assert_parse_err!("10.0.2-", "expected `alpha` or `rc` after `-`");
    assert_parse_err!("10.0.2-alpha", "expected `.` after `-alpha`");
    assert_parse_err!("10.0.2-alpha.", "expected digit after `-alpha.`");
    assert_parse_err!(
        "10.0.2-alpha.255",
        "`-alpha` build number is larger than 63"
    );
    assert_parse_err!("10.0.2-rc", "expected `.` after `-rc`");
    assert_parse_err!("10.0.2-rc.", "expected digit after `-rc.`");
    assert_parse_err!("10.0.2-rc.255", "`-rc` build number is larger than 63");
    assert_parse_err!("10.0.2-alpha.1+", "expected `dev` after `+`");
    assert_parse_err!("10.0.2-rc.1+dev", "unexpected `-rc` with `+dev`");
    assert_parse_err!("10.0.2+dev", "unexpected `+dev` without `-alpha`");
    assert_parse_err!(
        "10.0.2-alpha.1+dev extra_characters",
        "expected end of string"
    );
    assert_parse_err!("256.0.2-alpha.1+dev", "digit cannot be larger than 255");
    assert_parse_err!("10.256.2-alpha.1+dev", "digit cannot be larger than 255");
    assert_parse_err!("10.0.256-alpha.1+dev", "digit cannot be larger than 255");
    assert_parse_err!("10.0.2-alpha.256+dev", "digit cannot be larger than 255");
    assert_parse_err!(
        "01.0.2-alpha.256+dev",
        "multi-digit number cannot start with zero"
    );
    assert_parse_err!(
        "10.01.2-alpha.256+dev",
        "multi-digit number cannot start with zero"
    );
    assert_parse_err!(
        "10.0.01-alpha.256+dev",
        "multi-digit number cannot start with zero"
    );
    assert_parse_err!(
        "10.0.2-alpha.01+dev",
        "multi-digit number cannot start with zero"
    );
}
