//! TUID: Time-based Unique Identifiers.
//!
//! Time-ordered unique 128-bit identifiers.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "arrow2_convert",
    derive(arrow2_convert::ArrowSerialize, arrow2_convert::ArrowDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tuid {
    /// Approximate nanoseconds since epoch.
    time_ns: u64,

    /// Initialized to something random on each thread,
    /// then incremented for each new [`Tuid`] being allocated.
    inc: u64,
}

#[cfg(feature = "arrow2_convert")]
arrow2_convert::arrow_enable_vec_for_type!(Tuid);

// TODO(#1774): shouldn't have to write this manually
#[cfg(feature = "arrow2_convert")]
impl arrow2_convert::field::ArrowField for Tuid {
    type Type = Self;

    fn data_type() -> arrow2::datatypes::DataType {
        let datatype = arrow2::datatypes::DataType::Struct(<[_]>::into_vec(Box::new([
            <u64 as arrow2_convert::field::ArrowField>::field("time_ns"),
            <u64 as arrow2_convert::field::ArrowField>::field("inc"),
        ])));
        arrow2::datatypes::DataType::Extension("rerun.tuid".into(), Box::new(datatype), None)
    }
}

impl std::fmt::Display for Tuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032X}", self.as_u128())
    }
}

impl std::fmt::Debug for Tuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032X}", self.as_u128())
    }
}

impl Tuid {
    /// All zeroes.
    pub const ZERO: Self = Self { time_ns: 0, inc: 0 };

    /// All ones.
    pub const MAX: Self = Self {
        time_ns: u64::MAX,
        inc: u64::MAX,
    };

    #[inline]
    pub fn random() -> Self {
        use std::cell::RefCell;

        thread_local! {
            pub static LATEST_TUID: RefCell<Tuid> = RefCell::new(Tuid{
                time_ns: monotonic_nanos_since_epoch(),

                // Leave top bit at zero so we have plenty of room to grow.
                inc: random_u64() & !(1_u64 << 63),
            });
        }

        LATEST_TUID.with(|latest_tuid| {
            let mut latest = latest_tuid.borrow_mut();

            let new = Tuid {
                time_ns: monotonic_nanos_since_epoch(),
                inc: latest.inc + 1,
            };

            debug_assert!(
                latest.time_ns <= new.time_ns,
                "Time should be monotonically increasing"
            );

            *latest = new;

            new
        })
    }

    #[inline]
    pub fn as_u128(&self) -> u128 {
        ((self.time_ns as u128) << 64) | (self.inc as u128)
    }

    #[inline]
    pub fn nanoseconds_since_epoch(&self) -> u64 {
        self.time_ns
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
    use once_cell::sync::Lazy;
    use web_time::Instant;

    static START_TIME: Lazy<(u64, Instant)> = Lazy::new(|| (nanos_since_epoch(), Instant::now()));
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
    getrandom::getrandom(&mut bytes).expect("Couldn't get random bytes");
    u64::from_le_bytes(bytes)
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
    let ids: Vec<Tuid> = (0..num).map(|_| Tuid::random()).collect();
    assert!(is_sorted(&ids));
    assert_eq!(ids.iter().cloned().collect::<HashSet::<Tuid>>().len(), num);
    assert_eq!(ids.iter().cloned().collect::<BTreeSet::<Tuid>>().len(), num);
}
