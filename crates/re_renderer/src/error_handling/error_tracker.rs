use ahash::HashSet;
use parking_lot::Mutex;
use std::sync::{
    atomic::Ordering,
    atomic::{AtomicI64, AtomicUsize},
};

use super::handle_async_error;

#[cfg(not(webgpu))]
use super::wgpu_core_error::WrappedContextError;

#[cfg(webgpu)]
#[derive(Hash, PartialEq, Eq, Debug)]
struct WrappedContextError(pub String);

/// Coalesces wgpu errors until the tracker is `clear()`ed.
///
/// Used to avoid spamming the user with repeating errors.
/// [`RendererContext`] maintains a "top level" error tracker for all otherwise unhandled errors,
/// but error scopes can use their own error trackers.
pub struct ErrorTracker {
    tick_nr: AtomicUsize,

    /// This countdown reaching 0 indicates that the error section has stabilized into a
    /// sane state, which might take a few frames if we've just left a poisoned state.
    ///
    /// We use this to know when it makes sense to clear the error tracker.
    clear_countdown: AtomicI64,
    errors: Mutex<HashSet<WrappedContextError>>,
}

impl Default for ErrorTracker {
    fn default() -> Self {
        Self {
            tick_nr: AtomicUsize::new(0),
            clear_countdown: AtomicI64::new(i64::MAX),
            errors: Default::default(),
        }
    }
}

impl ErrorTracker {
    /// Increment tick count used in logged errors, and clear the tracker as needed.
    pub fn tick(&self) {
        self.tick_nr.fetch_add(1, Ordering::Relaxed);

        // The pipeline has stabilized back into a sane state, clear
        // the error tracker so that we're ready to log errors once again
        // if the pipeline gets back into a poisoned state.
        if self.clear_countdown.fetch_sub(1, Ordering::Relaxed) == 1 {
            self.clear_countdown.store(i64::MAX, Ordering::Relaxed);
            self.clear();
            re_log::info!("pipeline back into a sane state!");
        }
    }

    /// Resets the tracker.
    ///
    /// Call this when the pipeline is back into a sane state.
    pub fn clear(&self) {
        self.errors.lock().clear();
        re_log::debug!("cleared WGPU error tracker");
    }

    /// Handles an async error, logging it if needed.
    pub fn handle_error_future(
        self: &std::sync::Arc<Self>,
        error_scope_result: impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
    ) {
        handle_async_error(
            {
                let err_tracker = self.clone();
                move |error| err_tracker.handle_error(error)
            },
            error_scope_result,
        );
    }

    /// Logs a wgpu error, making sure to deduplicate them as needed.
    pub fn handle_error(&self, error: wgpu::Error) {
        // The pipeline is in a poisoned state, errors are still coming in: we won't be
        // clearing the tracker until it had at least 2 complete begin_frame cycles
        // without any errors (meaning the swapchain surface is stabilized).
        self.clear_countdown.store(3, Ordering::Relaxed);

        match error {
            wgpu::Error::OutOfMemory { source: _ } => {
                re_log::error!("A wgpu operation caused out-of-memory: {error}");
            }
            wgpu::Error::Validation {
                source: _source,
                description,
            } => {
                cfg_if::cfg_if! {
                    if #[cfg(webgpu)] {
                        if !self
                            .errors
                            .lock()
                            .insert(WrappedContextError(description.clone()))
                        {
                            // We've already logged this error since we've entered the
                            // current poisoned state. Don't log it again.
                            return;
                        }
                        re_log::error!(
                            tick_nr = self.tick_nr.load(Ordering::Relaxed),
                            %description,
                            "WGPU error",
                        );
                    } else {
                        match _source.downcast::<wgpu_core::error::ContextError>() {
                            Ok(ctx_err) => {
                                if ctx_err
                                    .cause
                                    .downcast_ref::<wgpu_core::command::CommandEncoderError>()
                                    .is_some()
                                {
                                    // Actual command encoder errors never carry any meaningful
                                    // information: ignore them.
                                    return;
                                }

                                let ctx_err = WrappedContextError(ctx_err);
                                if !self.errors.lock().insert(ctx_err) {
                                    // We've already logged this error since we've entered the
                                    // current poisoned state. Don't log it again.
                                    return;
                                }

                                re_log::error!(
                                    tick_nr = self.tick_nr.load(Ordering::Relaxed),
                                    %description,
                                    "WGPU error",
                                );
                            }
                            Err(err) => re_log::error!("Wgpu operation failed: {err}"),
                        }
                    }
                }
            }
        }
    }
}
