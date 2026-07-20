//! This module provides a simple exponential back-off generator with jitter (exponent 2, custom base).
//!
//! Jitter uses the "full jitter" strategy: each backoff sleeps for a random duration in
//! `[0, base)` (with the default jitter factor). This de-synchronizes concurrent clients retrying
//! the same endpoint, avoiding a thundering herd.
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
//! // Full jitter: the actual sleep is somewhere in `[0, base)`.
//! assert!(b.jittered() <= b.base());
//! // sleep with:
//! // b.sleep().await;
//!
//! let expected_backoffs = [2, 4, 8, 8, 8];
//! for expected in expected_backoffs {
//!    let b = generator.gen_next();
//!    assert_eq!(b.base(), Duration::from_secs(expected));
//!    assert!(b.jittered() <= b.base());
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

/// Run a (possibly `!Send`) wasm future to completion on the local executor, exposing the wait as a
/// `Send` future.
///
/// `spawn_local` confines the `!Send` future to the single-threaded wasm executor; the future this
/// returns is just the oneshot `Receiver`, which *is* `Send` and never captures `f`. This lets
/// JS-backed futures be awaited from `Send`-bounded contexts (e.g. a backoff sleep threaded through
/// a DataFusion stream in `re_datafusion`).
///
/// This must be a plain `fn` returning `impl Future + Send` (not an `async fn`): an `async fn`
/// would keep `f` in its own generator state and so be `!Send`. Same technique as
/// `re_datafusion::wasm_compat::make_future_send`, duplicated here to avoid a new crate just for it.
#[cfg(target_arch = "wasm32")]
fn run_local<F>(f: F) -> impl std::future::Future<Output = ()> + Send
where
    F: std::future::Future<Output = ()> + 'static,
{
    use futures::FutureExt as _;

    let (tx, rx) = futures::channel::oneshot::channel::<()>();

    wasm_bindgen_futures::spawn_local(async move {
        f.await;
        // The receiver is gone if the caller stopped waiting; nothing to do then.
        tx.send(()).ok();
    });

    // If the spawned task is dropped before it signals, `rx` resolves to `Err`; either way we're
    // done waiting.
    rx.map(|_result| ())
}

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: Duration) {
    let millis = duration.as_millis() as i32;

    // The `setTimeout` + `JsFuture` dance is `!Send`; `run_local` bridges it to a `Send` future.
    run_local(async move {
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
    })
    .await;
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
    /// Default jitter factor: `1.0` means "full jitter", i.e. the sleep is uniformly random in
    /// `[0, base)`. See [`Self::new_with_custom_jitter`] for the precise meaning.
    pub const DEFAULT_JITTER_FACTOR: f64 = 1.0;

    /// Create a new `BackoffGenerator` with the given base and max durations.
    /// A random jitter will be added to the backoff duration with a
    /// [`Self::DEFAULT_JITTER_FACTOR`] jitter factor.
    pub fn new(base: Duration, max: Duration) -> Result<Self, String> {
        Self::new_with_custom_jitter(base, max, Self::DEFAULT_JITTER_FACTOR)
    }

    /// Create a new `BackoffGenerator` with the given base and max durations and a custom
    /// `jitter_factor` in `[0, 1]`.
    ///
    /// The jittered sleep is uniformly random in `[(1.0 - jitter_factor) * base, base)`:
    /// * `1.0` → `[0, base)` (full jitter, the default).
    /// * `0.0` → `[base, base]` (no jitter).
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
        // Full jitter: pick a random duration in `[(1.0 - jitter_factor) * base, base)`.
        // With the default `jitter_factor = 1.0` this is `[0, base)`, which de-synchronizes
        // concurrent clients retrying the same endpoint (avoids a thundering herd).
        let rand = rand::random::<f64>(); // [0, 1)
        let factor = (1.0 - self.jitter_factor) + self.jitter_factor * rand; // [1 - jitter_factor, 1)
        let jittered_secs = duration.as_secs_f64() * factor;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_jitter_stays_within_zero_and_base() {
        let mut generator =
            BackoffGenerator::new(Duration::from_secs(1), Duration::from_secs(8)).unwrap();

        // Exponential bases, clamped to `max`.
        let expected_bases = [1, 2, 4, 8, 8, 8];
        for expected in expected_bases {
            // Sample a few times to exercise the randomness.
            for _ in 0..100 {
                let mut g = BackoffGenerator::new(generator.base, generator.max).unwrap();
                g.iteration = generator.iteration;
                let b = g.gen_next();
                assert_eq!(b.base(), Duration::from_secs(expected));
                // Full jitter: `[0, base)`.
                assert!(b.jittered() <= b.base());
            }
            generator.gen_next();
        }
    }

    #[test]
    fn zero_jitter_factor_yields_exactly_base() {
        let mut generator = BackoffGenerator::new_with_custom_jitter(
            Duration::from_millis(100),
            Duration::from_secs(1),
            0.0,
        )
        .unwrap();

        for _ in 0..100 {
            let b = generator.gen_next();
            assert_eq!(b.jittered(), b.base());
            generator.reset();
        }
    }
}
