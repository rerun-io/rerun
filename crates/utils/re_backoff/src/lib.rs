//! This module provides a simple exponential back-off generator with jitter (exponent 2, custom base).
//!
//! ### Example
//!
//! ```
//! use std::time::Duration;
//!
//! use re_backoff::BackoffGenerator;
//!
//! let mut generator = BackoffGenerator::new(Duration::from_secs(1), Duration::from_secs(8)).expect("valid generator");
//!
//! let b = generator.gen_next();
//! assert_eq!(b.base(), Duration::from_secs(1));
//! assert!(b.jittered() >= Duration::from_secs(1) && b.jittered() <= Duration::from_secs_f64(1.0 + 0.5));
//! // sleep with:
//! // b.sleep().await;
//!
//! let expected_backoffs = [2, 4, 8, 8, 8];
//! for expected in expected_backoffs {
//!    let b = generator.gen_next();
//!    assert_eq!(b.base(), Duration::from_secs(expected));
//!    assert!(b.jittered() >= Duration::from_secs(expected) && b.jittered() <= Duration::from_secs(expected + expected / 2));
//! }
//! ```

use std::time::Duration;

/// `Backoff` represent an await-able back-off duration.
///
/// It is normally built by a [`BackoffGenerator`].
#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    jittered: Duration,
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: Duration) {
    // Hack to get async sleep on wasm
    async fn sleep_ms(millis: i32) {
        let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
            web_sys::window()
                .expect("Failed to get window")
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
                .expect("Failed to call set_timeout");
        };
        let p = js_sys::Promise::new(&mut cb);
        wasm_bindgen_futures::JsFuture::from(p)
            .await
            .expect("Failed to await sleep promise");
    }

    sleep_ms(duration.as_millis() as i32).await;
}

impl Backoff {
    /// Sleep for the amount of time specified by this backoff instance.
    #[inline]
    pub async fn sleep(&self) {
        sleep(self.jittered).await;
    }

    /// The base duration for the backoff.
    ///
    /// Note that this is not the actual time this backoff will sleep for.
    /// However, access to this value is useful for debugging and logging.
    ///
    /// See [`Self::jittered()`] for the actual time.
    #[inline]
    pub fn base(&self) -> Duration {
        self.base
    }

    /// The actual time this backoff will sleep for, which is the [`Self::base()`] plus
    /// a random jitter.
    #[inline]
    pub fn jittered(&self) -> Duration {
        self.jittered
    }
}

/// `BackoffGenerator` is a generator for exponential back-off durations with jitter.
///  See module-level docs for an example how to use it.
#[derive(Debug)]
pub struct BackoffGenerator {
    base: Duration,
    max: Duration,
    jitter_factor: f64,
    iteration: u32,
}

impl BackoffGenerator {
    pub const DEFAULT_JITTER_FACTOR: f64 = 0.5;

    /// Create a new `BackoffGenerator` with the given base and max durations.
    /// A random jitter will be added to the backoff duration with a
    /// [`Self::DEFAULT_JITTER_FACTOR`] jitter factor.
    pub fn new(base: Duration, max: Duration) -> Result<Self, String> {
        Self::new_with_custom_jitter(base, max, Self::DEFAULT_JITTER_FACTOR)
    }

    /// Create a new `BackoffGenerator` with the given base and max durations.
    /// A random jitter will be added to the backoff duration with a
    /// custom `jitter_factor`.
    pub fn new_with_custom_jitter(
        base: Duration,
        max: Duration,
        jitter_factor: f64,
    ) -> Result<Self, String> {
        if base > max {
            return Err("base duration must be less than or equal to max duration".to_owned());
        }
        if jitter_factor < 0.0 || jitter_factor > 1.0 {
            return Err("jitter factor must be between 0 and 1".to_owned());
        }
        Ok(Self {
            base,
            max,
            jitter_factor,
            iteration: 0,
        })
    }

    fn jitter(&self, duration: Duration) -> Duration {
        // between 0 and self.jitter_factor
        let jitter = rand::random::<f64>() * self.jitter_factor;
        let jittered_secs = duration.as_secs_f64() * (1.0 + jitter);
        Duration::try_from_secs_f64(jittered_secs).unwrap_or(duration)
    }

    /// Generate the next back-off value.
    ///
    /// This will return a [`Backoff`] object. Call [`Backoff::sleep()`] to
    /// get a `Future` that can sleep for the duration.
    pub fn gen_next(&mut self) -> Backoff {
        let base = 2u32
            .checked_pow(self.iteration)
            .and_then(|p| self.base.checked_mul(p))
            .unwrap_or(self.max)
            .clamp(self.base, self.max);
        let jittered = self.jitter(base);

        self.iteration += 1;
        Backoff { base, jittered }
    }

    /// Generate the max back-off value (plus jitter).
    ///
    /// This doesn't advance the state of the generator
    pub fn max_backoff(&self) -> Backoff {
        let jittered = self.jitter(self.max);
        Backoff {
            base: self.max,
            jittered,
        }
    }

    /// Check whether the generator is at the initial state.
    pub fn is_reset(&self) -> bool {
        self.iteration == 0
    }

    /// Reset this generator to the initial state.
    pub fn reset(&mut self) {
        self.iteration = 0;
    }
}
