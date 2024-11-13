/// The number of time units per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timescale(u64);

impl Timescale {
    pub(crate) fn new(v: u64) -> Self {
        Self(v)
    }
}

/// A value in time units.
#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(pub i64);

impl Time {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(i64::MAX);
    pub const MIN: Self = Self(i64::MIN);

    /// Create a new value in _time units_.
    ///
    /// ⚠️ Don't use this for regular timestamps in seconds/milliseconds/etc.,
    /// use the proper constructors for those instead!
    /// This only exists for cases where you already have a value expressed in time units,
    /// such as those received from the `WebCodecs` APIs.
    #[inline]
    pub fn new(v: i64) -> Self {
        Self(v)
    }

    #[inline]
    pub fn from_secs(secs_since_start: f64, timescale: Timescale) -> Self {
        Self((secs_since_start * timescale.0 as f64).round() as i64)
    }

    #[inline]
    pub fn from_millis(millis_since_start: f64, timescale: Timescale) -> Self {
        Self::from_secs(millis_since_start / 1e3, timescale)
    }

    #[inline]
    pub fn from_micros(micros_since_start: f64, timescale: Timescale) -> Self {
        Self::from_secs(micros_since_start / 1e6, timescale)
    }

    #[inline]
    pub fn from_nanos(nanos_since_start: i64, timescale: Timescale) -> Self {
        Self::from_secs(nanos_since_start as f64 / 1e9, timescale)
    }

    /// Convert to a duration
    #[inline]
    pub fn duration(self, timescale: Timescale) -> std::time::Duration {
        std::time::Duration::from_nanos(self.into_nanos(timescale) as _)
    }

    #[inline]
    pub fn into_secs(self, timescale: Timescale) -> f64 {
        self.0 as f64 / timescale.0 as f64
    }

    #[inline]
    pub fn into_millis(self, timescale: Timescale) -> f64 {
        self.into_secs(timescale) * 1e3
    }

    #[inline]
    pub fn into_micros(self, timescale: Timescale) -> f64 {
        self.into_secs(timescale) * 1e6
    }

    #[inline]
    pub fn into_nanos(self, timescale: Timescale) -> i64 {
        (self.into_secs(timescale) * 1e9).round() as i64
    }
}

impl std::fmt::Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::ops::Add for Time {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for Time {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}
