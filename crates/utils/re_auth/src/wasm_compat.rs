//! Some parts of `re_auth` deal with `async`, `dyn Future`, and `Send`.
//! On Wasm, these things are a nightmare. So we use hacks to force futures to be `Send`.
//!
//! In theory, everything is `Send` on `wasm32`, because it's single-threaded.
//! If Wasm ever gets threads, it will be under a different `target_os`, such as `wasi`.
//!
//! Note: Bad decisions lie ahead.

/// Force a future to implement `Send`. Only available on `Wasm`.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) struct ForceSendFuture<F>(F);

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[expect(unsafe_code)]
/// SAFETY: Only used on `wasm32-unknown-unknown`, which implies single-threaded.
unsafe impl<F> Send for ForceSendFuture<F> {}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl<F: Future> Future for ForceSendFuture<F> {
    type Output = F::Output;

    #[expect(unsafe_code)] // Needed to project the pin onto `self.0`
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // SAFETY: We're not moving out of anything.
        let inner = unsafe { std::pin::Pin::new_unchecked(&mut self.get_unchecked_mut().0) };
        inner.poll(cx)
    }
}

/// Force a future to be `Send` if compiling for Wasm.
///
/// Wasm often involves dealing with `JsValue`, which is not `Send`.
/// Some public APIs expose this issue, and there is no good way to
/// propagate `Send`-ness throughout a codebase otherwise which uses `dyn`.
///
/// Do not attempt to store the resulting future. Instead, await it directly:
/// ```rust,ignore
/// make_future_send_on_wasm(fut).await
/// ```
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub(crate) fn make_future_send_on_wasm<F>(fut: F) -> F {
    fut
}

/// Force a future to be `Send` if compiling for Wasm.
///
/// Wasm often involves dealing with `JsValue`, which is not `Send`.
/// Some public APIs expose this issue, and there is no good way to
/// propagate `Send`-ness throughout a codebase otherwise which uses `dyn`.
///
/// Do not attempt to store the resulting future. Instead, await it directly:
/// ```rust,ignore
/// make_future_send_on_wasm(fut).await
/// ```
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) fn make_future_send_on_wasm<F>(fut: F) -> ForceSendFuture<F> {
    ForceSendFuture(fut)
}

/// Trait only requires `Send` on the wasm32 target.
///
/// Can be used as a "conditional `Send`" in trait bounds.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub(crate) trait SendIfNotWasm: Send {}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
impl<T: Send> SendIfNotWasm for T {}

/// Trait only requires `Send` on the wasm32 target.
///
/// Can be used as a "conditional `Send`" in trait bounds.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) trait SendIfNotWasm {}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl<T> SendIfNotWasm for T {}
