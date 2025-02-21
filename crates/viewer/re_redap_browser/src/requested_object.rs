use crossbeam_channel::{bounded, Receiver};

use re_viewer_context::{AsyncRuntimeHandle, WasmNotSend};

/// A handle to an object that is requested asynchronously.
pub enum RequestedObject<T: Send + 'static> {
    Pending(Receiver<T>),
    Completed(T),
}

impl<T: Send + 'static> RequestedObject<T> {
    /// Create a new [`Self`] with the given future.
    pub fn new<F>(runtime: &AsyncRuntimeHandle, func: F) -> Self
    where
        F: std::future::Future<Output = T> + WasmNotSend + 'static,
    {
        let (tx, rx) = bounded(1);
        let handle = Self::Pending(rx);

        runtime.spawn_future(async move {
            let result = func.await;
            let _ = tx.send(result);
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
        F: std::future::Future<Output = T> + WasmNotSend + 'static,
    {
        Self::new(runtime, async move {
            let result = func.await;
            egui_ctx.request_repaint();
            result
        })
    }

    /// Check if the future has completed and, if so, update our state to [`Self::Completed`].
    pub fn on_frame_start(&mut self) {
        let result = match self {
            Self::Pending(rx) => rx.try_recv().ok(),
            Self::Completed(_) => None,
        };

        if let Some(result) = result {
            *self = Self::Completed(result);
        }
    }

    /// Get a reference to the received object, if the request has completed.
    pub fn try_as_ref(&self) -> Option<&T> {
        match self {
            Self::Pending(_) => None,
            Self::Completed(result) => Some(result),
        }
    }
}
