use std::sync::Arc;

use parking_lot::Mutex;

use re_viewer_context::AsyncRuntimeHandle;

/// A handle to an object that is requested asynchronously.
pub enum RequestedObject<T: Send + 'static> {
    Pending(Arc<Mutex<Option<T>>>),
    Completed(T),
}

impl<T: Send + 'static> RequestedObject<T> {
    /// Create a new [`Self`] with the given future.
    pub fn new<F>(runtime: &AsyncRuntimeHandle, func: F) -> Self
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        let result = Arc::new(Mutex::new(None));
        let handle = Self::Pending(result.clone());

        runtime.spawn_future(async move {
            let r = func.await;
            result.lock().replace(r);
            //TODO: refresh egui?
        });

        handle
    }

    /// Check if the future has completed and, if so, update our state to [`Self::Completed`].
    pub fn on_frame_start(&mut self) {
        let result = match self {
            Self::Pending(handle) => handle.lock().take(),
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
