use ahash::HashMap;
use parking_lot::Mutex;

use super::handle_async_error;

#[cfg(not(webgpu))]
use super::wgpu_core_error::WrappedContextError;

#[cfg(webgpu)]
#[derive(Hash, PartialEq, Eq, Debug)]
struct WrappedContextError(pub String);

struct ErrorEntry {
    /// Frame index for frame on which this error was first logged.
    ///
    /// Since errors are received on the device timeline, not the content timeline
    /// this may be arbitrarily in the past!
    frame_index: u64,
}

/// Keeps track of wgpu errors and de-duplicates messages across frames.
///
/// Unless we're running without wgpu-core (i.e. using `WebGPU`), what accounts for as an error duplicate is
/// a heuristic based on wgpu-core error type.
///
/// Tracker can be manually cleared, otherwise it will clear errors individually if they don't show up for a frame.
/// Note that since errors resolve in the device timeline (see <https://www.w3.org/TR/webgpu/#programming-model-timelines>)
/// error receiving and error clearing may be arbitrarily delayed!
///
/// Used to avoid spamming the user with repeating errors.
/// [`RendererContext`] maintains a "top level" error tracker for all otherwise unhandled errors,
/// but error scopes can use their own error trackers.
#[derive(Default)]
pub struct ErrorTracker {
    errors: Mutex<HashMap<WrappedContextError, ErrorEntry>>,
}

impl ErrorTracker {
    /// Called by the renderer context when the last error scope of a frame has finished.
    ///
    /// Error scopes live on the device timeline, which may be arbitrarily delayed compared to the content timeline.
    /// See <https://www.w3.org/TR/webgpu/#programming-model-timelines>.
    pub(crate) fn on_device_timeline_frame_finished(&self, device_timeline_frame_index: u64) {
        let mut errors = self.errors.lock();
        errors.retain(|_error, entry| {
            // If the error was not logged on the just concluded frame, remove it.
            device_timeline_frame_index == entry.frame_index
        });
    }

    /// Handles an async error, calling [`ErrorTracker::handle_error`] as needed.
    ///
    /// `frame_index` should be the currently active frame index which is associated with the scope.
    /// (by the time the scope finishes, the active frame index may have changed)
    #[allow(unused)] // TODO(andreas): Currently unused, but this should be exposed to re_renderer users to handle smaller scopes.
    pub fn handle_error_future(
        self: &std::sync::Arc<Self>,
        error_scope_result: impl IntoIterator<
            Item = impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
        >,
        frame_index: u64,
    ) {
        self.handle_error_future_with_callback(error_scope_result, frame_index, |_, _| {});
    }

    /// Like [`Self::handle_error_future`], but additionally end calls the passed callback when
    /// the last passed future has resolved.
    ///
    /// The callback gets passed the error tracker and the frame index.
    pub(crate) fn handle_error_future_with_callback(
        self: &std::sync::Arc<Self>,
        error_scope_result: impl IntoIterator<
            Item = impl std::future::Future<Output = Option<wgpu::Error>> + Send + 'static,
        >,
        frame_index: u64,
        on_last_scope_resolved: impl Fn(&Self, u64) + Send + Sync + 'static,
    ) {
        let mut error_scope_result = error_scope_result.into_iter().peekable();
        while let Some(error_future) = error_scope_result.next() {
            if error_scope_result.peek().is_none() {
                let err_tracker = self.clone();
                handle_async_error(
                    move |error| {
                        if let Some(error) = error {
                            err_tracker.handle_error(error, frame_index);
                        }
                        on_last_scope_resolved(&err_tracker, frame_index);
                    },
                    error_future,
                );
                break;
            }

            let err_tracker = self.clone();
            handle_async_error(
                move |error| {
                    if let Some(error) = error {
                        err_tracker.handle_error(error, frame_index);
                    }
                },
                error_future,
            );
        }
    }

    /// Logs a wgpu error to the tracker.
    ///
    /// If the error happened in the previous (device timeline) frame already, it will be deduplicated.
    ///
    /// `frame_index` should be the frame index associated with the error scope.
    /// Since errors are reported on the device timeline, not the content timeline,
    /// this may not be the currently active frame index!
    pub fn handle_error(&self, error: wgpu::Error, frame_index: u64) {
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
                        if self.errors.lock().insert(
                            WrappedContextError(description.clone()),
                            ErrorEntry { frame_index }
                        ).is_none() {
                            re_log::error!(
                                "WGPU error in frame {}: {}", frame_index, description
                            );
                        }
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
                                if self.errors.lock().insert(ctx_err, ErrorEntry { frame_index }).is_none() {
                                    re_log::error!(
                                        "WGPU error in frame {}: {}", frame_index, description
                                    );
                                }
                            }
                            Err(err) => re_log::error!("Wgpu operation failed: {err}"),
                        }
                    }
                }
            }
        }
    }
}
