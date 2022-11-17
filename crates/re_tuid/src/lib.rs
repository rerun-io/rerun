#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Tuid {
    /// Approximate nanoseconds since epoch.
    ns_since_epoch: u64,

    /// Initialized to something random on each thread,
    /// then incremented for each new Tuid being allocated.
    randomness: u64,
}

impl Tuid {
    /// All zeroes.
    pub const ZERO: Self = Self {
        ns_since_epoch: 0,
        randomness: 0,
    };

    /// All ones.
    pub const MAX: Self = Self {
        ns_since_epoch: u64::MAX,
        randomness: u64::MAX,
    };

    #[inline]
    #[cfg(not(target_arch = "wasm32"))] // TODO(emilk): implement for wasm32 (needs ms since epoch).
    pub fn random() -> Self {
        use std::cell::RefCell;

        thread_local! {
            pub static LATEST_TUID: RefCell<Tuid> = RefCell::new(Tuid{
                ns_since_epoch: monotonic_nanos_since_epoch(),

                // Leave top bit at zero so we have plenty of room to grow.
                randomness: random_u64() & !(1_u64 << 63),
            });
        }

        LATEST_TUID.with(|latest_tuid| {
            let mut latest = latest_tuid.borrow_mut();

            let new = Tuid {
                ns_since_epoch: monotonic_nanos_since_epoch(),
                randomness: latest.randomness + 1,
            };

            debug_assert!(
                latest.ns_since_epoch <= new.ns_since_epoch,
                "Time should be monotonically increasing"
            );

            *latest = new;

            new
        })
    }

    #[inline]
    pub fn as_u128(&self) -> u128 {
        ((self.ns_since_epoch as u128) << 64) | (self.randomness as u128)
    }
}

#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for Tuid {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.ns_since_epoch ^ self.randomness);
    }
}

#[cfg(feature = "nohash_hasher")]
impl nohash_hasher::IsEnabled for IndexHash {}

/// Returns a high-precision, monotonically increasing count that approximates nanoseconds since unix epoch.
#[inline]
#[cfg(not(target_arch = "wasm32"))]
fn monotonic_nanos_since_epoch() -> u64 {
    // This can maybe be optimized
    use once_cell::sync::Lazy;
    use std::time::Instant;

    fn epoch_offset_and_start() -> (u64, Instant) {
        if let Ok(duration_since_epoch) = std::time::UNIX_EPOCH.elapsed() {
            let nanos_since_epoch = duration_since_epoch.as_nanos() as u64;
            (nanos_since_epoch, Instant::now())
        } else {
            // system time is set before 1970. this should be quite rare.
            (0, Instant::now())
        }
    }

    static START_TIME: Lazy<(u64, Instant)> = Lazy::new(epoch_offset_and_start);
    START_TIME.0 + START_TIME.1.elapsed().as_nanos() as u64
}

#[inline]
#[cfg(not(target_arch = "wasm32"))]
fn random_u64() -> u64 {
    let mut bytes = [0_u8; 8];
    getrandom::getrandom(&mut bytes).expect("Couldn't get randomness");
    u64::from_le_bytes(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
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
