//! TUID: Time-based Unique Identifiers.
//!
//! Time-ordered unique 128-bit identifiers.
//!
//! ## Format
//! The default string format is big-endian hex, e.g. `182342300C5F8C327a7b4a6e5a379ac4`.
//! This means the string representation sorts the same.
//!
//! ## Namespace prefix
//! It is common to prefix an TUID with a _namespace_. This is done as:
//! `{namespace}_{tuid}` where `namespace` can be anything but is _recommended_ to be:
//! * Lowercase
//! * ASCII
//! * Short ("row", "user", "chunk", …)
//!
//! For instance, `user_182342300C5F8C327a7b4a6e5a379ac4`.
//!
//! The idiomatic way of implementing this is to wrap [`Tuid`] in a newtype struct
//! (e.g. `struct UserId(Tuid)`) and implement the prefix there.
//!
//! It is recommended that
//! * Finding the wrong prefix is an error
//! * A missing prefix is NOT an error
//!
//! Thus, `user_182342300C5F8C327a7b4a6e5a379ac4` and `182342300C5F8C327a7b4a6e5a379ac4`
//! are both valid `UserId`:s, but `chunk_182342300C5F8C327a7b4a6e5a379ac4` is NOT.
//!
//! The namespace if ONLY part of the _string_ representation, and is there to help
//! a user identify what would otherwise be just random hex.
//! In other words, it's mainly for _debugging_ purposes.
//!
//! When storing the TUID in e.g. an Arrow column, use 16 bytes for each id.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

/// TUID: Time-based Unique Identifier.
///
/// Time-ordered globally unique 128-bit identifiers.
///
/// The raw bytes of the `Tuid` sorts in time order as the `Tuid` itself,
/// and the `Tuid` is byte-aligned so you can just transmute between `Tuid` and raw bytes.
#[repr(C, align(1))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[cfg_attr(
    feature = "bytemuck",
    derive(bytemuck::AnyBitPattern, bytemuck::NoUninit)
)]
pub struct Tuid {
    /// Approximate nanoseconds since epoch.
    ///
    /// A big-endian u64 encoded as bytes to keep the alignment of `Tuid` to 1.
    ///
    /// We use big-endian so that the raw bytes of the `Tuid` sorts in time order.
    time_nanos: [u8; 8],

    /// Initialized to something random on each thread,
    /// then incremented for each new [`Tuid`] being allocated.
    ///
    /// Uses big-endian u64 encoded as bytes to keep the alignment of `Tuid` to 1.
    ///
    /// We use big-endian so that the raw bytes of the `Tuid` sorts in creation order.
    inc: [u8; 8],
}

impl Tuid {
    /// We give an actual name to [`Tuid`], and inject that name into the Arrow datatype extensions,
    /// as a hack so that we can compactly format them when printing Arrow data to the terminal.
    /// Check out `re_arrow_util::format` for context.
    pub const ARROW_EXTENSION_NAME: &'static str = "rerun.datatypes.TUID";
}

/// Formats the [`Tuid`] as a hex string.
///
/// The format uses upper case for the first 16 hex digits, and lower case for the last 16 hex digits.
/// This is to make it easily distinguished from other hex strings.
///
/// Example: `182342300C5F8C327a7b4a6e5a379ac4`
impl std::fmt::Display for Tuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016X}{:016x}", self.nanos_since_epoch(), self.inc())
    }
}

impl std::str::FromStr for Tuid {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u128::from_str_radix(s, 16).map(Self::from_u128)
    }
}

impl std::fmt::Debug for Tuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl From<Tuid> for std::borrow::Cow<'_, Tuid> {
    #[inline]
    fn from(value: Tuid) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Tuid> for std::borrow::Cow<'a, Tuid> {
    #[inline]
    fn from(value: &'a Tuid) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl Tuid {
    /// All zeroes.
    pub const ZERO: Self = Self {
        time_nanos: [0; 8],
        inc: [0; 8],
    };

    /// All ones.
    pub const MAX: Self = Self {
        time_nanos: u64::MAX.to_be_bytes(),
        inc: u64::MAX.to_be_bytes(),
    };

    /// Create a new unique [`Tuid`] based on the current time.
    #[expect(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        use std::cell::RefCell;

        thread_local! {
            pub static LATEST_TUID: RefCell<Tuid> = RefCell::new(Tuid::from_nanos_and_inc(
                 monotonic_nanos_since_epoch(),

                // Leave top bit at zero so we have plenty of room to grow.
                 random_u64() & !(1_u64 << 63),
            ));
        }

        LATEST_TUID.with(|latest_tuid| {
            let mut latest = latest_tuid.borrow_mut();

            let new = Self::from_nanos_and_inc(monotonic_nanos_since_epoch(), latest.inc() + 1);

            debug_assert!(
                latest.nanos_since_epoch() <= new.nanos_since_epoch(),
                "Time should be monotonically increasing"
            );

            *latest = new;

            new
        })
    }

    /// Construct a [`Tuid`] from the upper and lower halves of a u128-bit.
    /// The first should be nano-seconds since epoch.
    #[inline]
    pub fn from_nanos_and_inc(time_nanos: u64, inc: u64) -> Self {
        Self {
            time_nanos: time_nanos.to_be_bytes(),
            inc: inc.to_be_bytes(),
        }
    }

    #[inline]
    pub fn from_u128(id: u128) -> Self {
        Self::from_nanos_and_inc((id >> 64) as u64, (id & (!0 >> 64)) as u64)
    }

    #[cfg(feature = "bytemuck")]
    #[inline]
    pub fn slice_from_bytes(bytes: &[u8]) -> Result<&[Self], bytemuck::PodCastError> {
        bytemuck::try_cast_slice(bytes)
    }

    #[inline]
    pub fn as_u128(&self) -> u128 {
        ((self.nanos_since_epoch() as u128) << 64) | (self.inc() as u128)
    }

    #[inline]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self::from_u128(u128::from_be_bytes(bytes))
    }

    /// Returns most significant byte first (big endian).
    #[inline]
    pub fn as_bytes(&self) -> [u8; 16] {
        self.as_u128().to_be_bytes()
    }

    /// Approximate nanoseconds since unix epoch.
    ///
    /// The upper 64 bits of the [`Tuid`].
    #[inline]
    pub fn nanos_since_epoch(&self) -> u64 {
        u64::from_be_bytes(self.time_nanos)
    }

    /// The increment part of the [`Tuid`].
    ///
    /// The lower 64 bits of the [`Tuid`].
    #[inline]
    pub fn inc(&self) -> u64 {
        u64::from_be_bytes(self.inc)
    }

    /// Returns the next logical [`Tuid`].
    ///
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`Tuid::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn next(&self) -> Self {
        let Self { time_nanos, inc } = *self;

        Self {
            time_nanos,
            inc: u64::from_be_bytes(inc).wrapping_add(1).to_be_bytes(),
        }
    }

    /// Returns the `n`-next logical [`Tuid`].
    ///
    /// This is equivalent to calling [`Tuid::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`Tuid::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn incremented_by(&self, n: u64) -> Self {
        let Self { time_nanos, inc } = *self;
        Self {
            time_nanos,
            inc: u64::from_be_bytes(inc).wrapping_add(n).to_be_bytes(),
        }
    }

    /// A shortened string representation of the `Tuid`.
    #[inline]
    pub fn short_string(&self) -> String {
        // We still want this to look like a part of the full TUID (i.e. what is printed on
        // `std::fmt::Display`).
        // Per Thread randomness plus increment is in the last part, so show only that.
        // (the first half is time in nanoseconds which for the _most part_ doesn't change that
        // often)
        let str = self.to_string();
        str[(str.len() - 8)..].to_string()
    }
}

/// Returns a high-precision, monotonically increasing count that approximates nanoseconds since unix epoch.
#[inline]
fn monotonic_nanos_since_epoch() -> u64 {
    // This can maybe be optimized

    use web_time::Instant;

    static START_TIME: std::sync::LazyLock<(u64, Instant)> =
        std::sync::LazyLock::new(|| (nanos_since_epoch(), Instant::now()));
    START_TIME.0 + START_TIME.1.elapsed().as_nanos() as u64
}

fn nanos_since_epoch() -> u64 {
    if let Ok(duration_since_epoch) = web_time::SystemTime::UNIX_EPOCH.elapsed() {
        let mut nanos_since_epoch = duration_since_epoch.as_nanos() as u64;

        if cfg!(target_arch = "wasm32") {
            // Web notriously round to the nearest millisecond (because of spectre/meltdown)
            // so we add a bit of extra randomenss here to increase our entropy and reduce the chance of collisions:
            nanos_since_epoch += random_u64() % 1_000_000;
        }

        nanos_since_epoch
    } else {
        // system time is set before 1970. this should be quite rare.
        0
    }
}

#[inline]
fn random_u64() -> u64 {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).expect("Couldn't get random bytes");
    u64::from_be_bytes(bytes)
}

impl re_byte_size::SizeBytes for Tuid {
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
fn test_tuid() {
    use std::collections::{BTreeSet, HashSet};

    fn is_sorted<T>(data: &[T]) -> bool
    where
        T: Ord,
    {
        data.windows(2).all(|w| w[0] <= w[1])
    }

    let num = 100_000;
    let mut ids = Vec::with_capacity(num);
    ids.push(Tuid::ZERO);
    ids.push(Tuid::from_nanos_and_inc(123_456, 789_123));
    ids.push(Tuid::from_nanos_and_inc(123_456, u64::MAX));
    ids.extend((0..num - 5).map(|_| Tuid::new()));
    ids.push(Tuid::from_nanos_and_inc(u64::MAX, 1));
    ids.push(Tuid::MAX);

    assert!(is_sorted(&ids));
    assert_eq!(ids.iter().copied().collect::<HashSet::<Tuid>>().len(), num);
    assert_eq!(ids.iter().copied().collect::<BTreeSet::<Tuid>>().len(), num);

    for &tuid in &ids {
        assert_eq!(tuid, Tuid::from_u128(tuid.as_u128()));
        assert_eq!(tuid, tuid.to_string().parse().unwrap());
    }

    let id_strings: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    assert!(
        is_sorted(&id_strings),
        "Ids should sort the same when converted to strings"
    );
}

#[test]
fn test_tuid_size_and_alignment() {
    assert_eq!(std::mem::size_of::<Tuid>(), 16);
    assert_eq!(std::mem::align_of::<Tuid>(), 1);
}

#[test]
fn test_tuid_formatting() {
    assert_eq!(
        Tuid::from_u128(0x182342300c5f8c327a7b4a6e5a379ac4).to_string(),
        "182342300C5F8C327a7b4a6e5a379ac4"
    );
}

// -------------------------------------------------------------------------------

// For backwards compatibility with our MsgPack encoder/decoder
#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
struct LegacyTuid {
    time_nanos: u64,
    inc: u64,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Tuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LegacyTuid {
            time_nanos: self.nanos_since_epoch(),
            inc: self.inc(),
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Tuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let LegacyTuid { time_nanos, inc } = serde::Deserialize::deserialize(deserializer)?;
        Ok(Self::from_nanos_and_inc(time_nanos, inc))
    }
}
