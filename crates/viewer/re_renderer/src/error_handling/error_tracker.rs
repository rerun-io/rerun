use ahash::HashMap;
use re_mutex::Mutex;

use super::wgpu_core_error::WgpuCoreWrappedContextError;

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum ContextError {
    WgpuCoreError(WgpuCoreWrappedContextError),
    #[cfg(web)]
    WebGpuError(String),
}

pub struct ErrorEntry {
    /// Frame index for frame on which this error was last logged.
    last_occurred_frame_index: u64,

    /// Description of the error.
    // TODO(#4507): Expecting to need this once we use this in views. Also very useful for debugging.
    #[expect(dead_code)]
    description: String,
}

/// Keeps track of wgpu errors and de-duplicates messages across frames.
///
/// On native & webgl, what accounts for as an error duplicate is a heuristic based on wgpu-core error type.
///
/// Used to avoid spamming the user with repeating errors.
/// [`crate::RenderContext`] maintains a "top level" error tracker for all otherwise unhandled errors.
///
/// TODO(#4507): Users should be able to create their own scopes feeding into separate trackers.
#[derive(Default)]
pub struct ErrorTracker {
    pub errors: Mutex<HashMap<ContextError, ErrorEntry>>,
}

impl ErrorTracker {
    /// Called by the renderer context when the last error scope of a frame has finished.
    ///
    /// Error scopes live on the device timeline, which may be arbitrarily delayed compared to the content timeline.
    /// See <https://www.w3.org/TR/webgpu/#programming-model-timelines>.
    /// Do *not* call this with the content pipeline's frame index!
    pub fn on_device_timeline_frame_finished(&self, device_timeline_frame_index: u64) {
        let mut errors = self.errors.lock();
        errors.retain(|_error, entry| {
            // If the error was not logged on the just concluded frame, remove it.
            device_timeline_frame_index == entry.last_occurred_frame_index
        });
    }

    /// Logs a wgpu error to the tracker.
    ///
    /// If the error happened already already, it will be deduplicated.
    ///
    /// `frame_index` should be the frame index associated with the error scope.
    /// Since errors are reported on the `device timeline`, not the `content timeline`,
    /// this may not be the currently active frame index!
    pub fn handle_error(&self, error: wgpu::Error, frame_index: u64) {
        let is_internal_error = matches!(error, wgpu::Error::Internal { .. });

        match error {
            wgpu::Error::OutOfMemory { source: _ } => {
                re_log::error!("A wgpu operation caused out-of-memory: {error}");
            }
            wgpu::Error::Internal {
                source,
                description,
            }
            | wgpu::Error::Validation {
                source,
                description,
            } => {
                let entry = ErrorEntry {
                    last_occurred_frame_index: frame_index,
                    description: description.clone(),
                };

                let should_log = match source.downcast::<wgpu::wgc::error::ContextError>() {
                    Ok(ctx_err) => {
                        if ctx_err
                            .source
                            .downcast_ref::<wgpu::wgc::command::CommandEncoderError>()
                            .is_some()
                        {
                            // Actual command encoder errors never carry any meaningful
                            // information: ignore them.
                            return;
                        }

                        // Don't log errors for texture creation errors, they are exposed
                        // as visualizer errors instead.
                        let is_texture_err = ctx_err
                            .source
                            .is::<wgpu::wgc::resource::CreateTextureError>();

                        let ctx_err =
                            ContextError::WgpuCoreError(WgpuCoreWrappedContextError(ctx_err));

                        let new_error = self.errors.lock().insert(ctx_err, entry).is_none();

                        !is_texture_err && new_error
                    }

                    #[cfg(not(web))]
                    Err(_) => true,

                    // We might be running with WebGPU on the web and therefore don't have a wgpu_core type.
                    #[cfg(web)]
                    Err(_) => {
                        let ctx_err = ContextError::WebGpuError(description.clone());
                        self.errors.lock().insert(ctx_err, entry).is_none()
                    }
                };

                if should_log {
                    let base_description = if is_internal_error {
                        "Internal wgpu error"
                    } else {
                        "Wgpu validation error"
                    };
                    re_log::error!("{base_description} {frame_index}: {description}");
                }
            }
        }
    }
}
