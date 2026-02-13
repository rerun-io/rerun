use std::fmt::Debug;

#[cfg(not(target_arch = "wasm32"))]
pub trait WasmNotSend: Send {}

#[cfg(target_arch = "wasm32")]
pub trait WasmNotSend {}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> WasmNotSend for T {}

#[cfg(target_arch = "wasm32")]
impl<T> WasmNotSend for T {}

#[derive(Debug, thiserror::Error)]
pub enum AsyncRuntimeError {
    /// Tokio returned an error.
    ///
    /// We cannot leak a tokio type, so we have to convert it to a string.
    #[error("Tokio error: {0}")]
    TokioError(String),
}

/// Thin abstraction over the async runtime.
///
/// This allows us to use tokio on native and the browser futures.
#[derive(Clone)]
pub struct AsyncRuntimeHandle {
    #[cfg(not(target_arch = "wasm32"))]
    tokio: tokio::runtime::Handle,
}

impl Debug for AsyncRuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncRuntimeHandle").finish()
    }
}

impl AsyncRuntimeHandle {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_native(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_web() -> Self {
        Self {}
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn inner(&self) -> &tokio::runtime::Handle {
        &self.tokio
    }

    /// Create an `AsyncRuntime` from the current tokio runtime on native.
    #[cfg_attr(target_arch = "wasm32", expect(clippy::unnecessary_wraps))]
    pub fn from_current_tokio_runtime_or_wasmbindgen() -> Result<Self, AsyncRuntimeError> {
        #[cfg(target_arch = "wasm32")]
        {
            Ok(Self::new_web())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(Self::new_native(
                tokio::runtime::Handle::try_current()
                    .map_err(|err| AsyncRuntimeError::TokioError(err.to_string()))?
                    .clone(),
            ))
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[expect(clippy::unused_self)]
    pub fn spawn_future<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + WasmNotSend + 'static,
    {
        wasm_bindgen_futures::spawn_local(future);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_future<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + WasmNotSend + 'static,
    {
        self.tokio.spawn(future);
    }
}
