/// Thin abstraction over the async runtime.
///
/// This allows us to use tokio on native and the browser futures.
#[derive(Clone)]
pub struct AsyncRuntimeHandle {
    #[cfg(not(target_arch = "wasm32"))]
    tokio: tokio::runtime::Handle,
}

impl AsyncRuntimeHandle {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_native(tokio: &tokio::runtime::Handle) -> Self {
        Self {
            tokio: tokio.clone(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_web() -> Self {
        Self {}
    }

    /// Create an `AsyncRuntime` from the current tokio runtime on native.
    // TODO: no anyhow plz. Can't leak tokio type though.
    pub fn from_current_tokio_runtime_or_wasmbindgen() -> anyhow::Result<Self> {
        #[cfg(target_arch = "wasm32")]
        {
            Ok(Self::new_web())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(Self::new_native(&tokio::runtime::Handle::try_current()?))
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[expect(unused_self)]
    pub fn spawn_future<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        wasm_bindgen_futures::spawn_local(future);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_future<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + 'static + Send,
    {
        self.tokio.spawn(future);
    }
}
