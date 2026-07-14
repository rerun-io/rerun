use crossbeam::channel::Receiver;
use re_viewer_context::{AsyncRuntimeHandle, WasmNotSend};

/// A handle to an object that is requested asynchronously.
///
/// Note: this object cannot be [`Clone`] because it uses a one-shot channel to track completion of
/// the async operation.
#[derive(Debug)]
pub enum RequestedObject<T: Send + 'static> {
    Pending {
        rx: Receiver<T>,
        previous: Option<T>,
        // TODO(grtlr): consider adding a timestamp for when the request was initiated.
        // This would allow us to show a loading spinner only after a certain amount of
        // time has passed, to avoid further UI flickers.
    },
    Completed(T),
}

impl<T: Send + 'static> RequestedObject<T> {
    /// Create a new [`Self`] with the given future.
    ///
    /// Optionally retains a `previous` value while the future is pending.
    pub fn new<F>(runtime: &AsyncRuntimeHandle, func: F, previous: Option<T>) -> Self
    where
        T: std::fmt::Debug,
        F: std::future::Future<Output = T> + WasmNotSend + 'static,
    {
        let (tx, rx) = crate::create_channel(1);
        let handle = Self::Pending { rx, previous };

        runtime.spawn_future(async move {
            //TODO(#9836): implement cancellation using another channel (see `make_future_send`)
            let result = func.await;
            re_quota_channel::send_crossbeam(&tx, result).ok();
        });

        handle
    }

    /// Create a new [`Self`] with the given future and automatically request a repaint of the UI
    /// when the future completes.
    pub fn new_with_repaint<F>(
        runtime: &AsyncRuntimeHandle,
        egui_ctx: egui::Context,
        func: F,
    ) -> Self
    where
        T: std::fmt::Debug,
        F: std::future::Future<Output = T> + WasmNotSend + 'static,
    {
        Self::new(
            runtime,
            async move {
                let result = func.await;
                egui_ctx.request_repaint();
                result
            },
            None,
        )
    }

    /// Refresh the requested object, retaining the latest available object while the new request is
    /// pending.
    pub fn refresh_with_previous_and_repaint<F>(
        self,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: egui::Context,
        func: F,
    ) -> Self
    where
        T: std::fmt::Debug,
        F: std::future::Future<Output = T> + WasmNotSend + 'static,
    {
        Self::new(
            runtime,
            async move {
                let result = func.await;
                egui_ctx.request_repaint();
                result
            },
            self.take_latest(),
        )
    }

    /// Check if the future has completed and, if so, update our state to [`Self::Completed`].
    pub fn on_frame_start(&mut self) {
        let result = match self {
            Self::Pending { rx, previous: _ } => rx.try_recv().ok(),
            Self::Completed(_) => None,
        };

        if let Some(result) = result {
            *self = Self::Completed(result);
        }
    }

    /// Get a reference to the latest available object.
    pub fn try_as_ref(&self) -> Option<&T> {
        match self {
            Self::Pending { rx: _, previous } => previous.as_ref(),
            Self::Completed(result) => Some(result),
        }
    }

    /// Take the latest available object, if any.
    pub fn take_latest(self) -> Option<T> {
        match self {
            Self::Pending { rx: _, previous } => previous,
            Self::Completed(result) => Some(result),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This test is to ensure you think twice before deriving `Clone` for [`RequestedObject`] (see
    /// docstring for the background).
    #[test]
    fn requested_object_not_clone() {
        static_assertions::assert_not_impl_any!(RequestedObject<usize>: Clone);
    }
}
