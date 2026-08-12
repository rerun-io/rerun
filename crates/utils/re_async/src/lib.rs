//! Async runtime abstractions for native and `WebAssembly` targets.

use std::fmt::Debug;
use std::future::Future;
use std::time::Duration;

mod read_at;

pub use read_at::AsyncReadAt;

/// Waits for at least `duration`.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// Waits for at least `duration` using the browser's timer queue.
///
/// Browser timers use signed 32-bit millisecond delays, so longer durations are clamped.
#[cfg(target_arch = "wasm32")]
pub async fn sleep(duration: Duration) {
    let milliseconds = i32::try_from(duration.as_millis()).unwrap_or(i32::MAX);
    spawn_local_with_result(async move {
        let mut callback = |resolve: js_sys::Function, _reject: js_sys::Function| {
            web_sys::window()
                .expect("browser window should exist")
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, milliseconds)
                .expect("browser timer should be created");
        };

        js_sys::Promise::new(&mut callback)
            .await
            .expect("browser timer should complete");
    })
    .await
    .expect("browser timer task should not be canceled while it is awaited");
}

/// Yields to the browser event loop so other tasks can run.
#[cfg(target_arch = "wasm32")]
#[inline]
pub async fn yield_now() {
    use wasm_bindgen::JsCast as _;

    // Use `scheduler.yield()`, if available. More information:
    // https://developer.mozilla.org/docs/Web/API/Scheduler/yield
    let global = js_sys::global();
    if let Ok(scheduler) = js_sys::Reflect::get(&global, &"scheduler".into())
        && let Ok(yield_fn) = js_sys::Reflect::get(&scheduler, &"yield".into())
        && let Some(yield_fn) = yield_fn.dyn_ref::<js_sys::Function>()
    {
        let promise = yield_fn
            .call0(&scheduler)
            .expect("scheduler.yield should return a promise")
            .dyn_into::<js_sys::Promise>()
            .expect("scheduler.yield should return a promise");
        promise.await.expect("scheduler.yield should complete");
    } else {
        sleep(Duration::ZERO).await;
    }
}

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
    #[error("Tokio error: {0}")]
    TokioError(String),
}

/// A handle to an async executor supplied by the application.
///
/// On native targets this wraps a Tokio runtime handle.
/// On `WebAssembly` it dispatches futures to the browser's local executor.
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

    /// Captures the current Tokio runtime on native or the browser executor on `WebAssembly`.
    #[cfg_attr(target_arch = "wasm32", expect(clippy::unnecessary_wraps))]
    pub fn from_current_tokio_runtime_or_wasmbindgen() -> Result<Self, AsyncRuntimeError> {
        cfg_select! {
            target_arch = "wasm32" => {
                Ok(Self::new_web())
            }
            _ => {
                Ok(Self::new_native(
                    tokio::runtime::Handle::try_current()
                        .map_err(|err| AsyncRuntimeError::TokioError(err.to_string()))?
                        .clone(),
                ))
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[expect(clippy::unused_self)]
    pub fn spawn_future<F>(&self, future: F)
    where
        F: Future<Output = ()> + WasmNotSend + 'static,
    {
        spawn_local(future);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_future<F>(&self, future: F)
    where
        F: Future<Output = ()> + WasmNotSend + 'static,
    {
        self.tokio.spawn(future);
    }
}

/// Indicates that a browser task was canceled before producing its result.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("browser task was canceled")]
pub struct TaskCancelled;

/// Spawns a future on the browser's local executor.
#[cfg(target_arch = "wasm32")]
#[inline]
#[expect(
    clippy::disallowed_methods,
    reason = "this is the workspace's browser executor boundary"
)]
pub fn spawn_local(future: impl Future<Output = ()> + 'static) {
    js_sys::futures::spawn_local(future);
}

/// Spawns a possibly non-`Send` browser future and returns a `Send` future for its result.
///
/// Dropping the returned future cancels the spawned future.
#[cfg(target_arch = "wasm32")]
pub fn spawn_local_with_result<F, T>(
    future: F,
) -> impl Future<Output = Result<T, TaskCancelled>> + Send + 'static
where
    F: Future<Output = T> + 'static,
    T: Send + 'static,
{
    use futures::TryFutureExt as _;
    use futures::future::{Either, select};
    use futures::pin_mut;

    let (mut sender, receiver) = futures::channel::oneshot::channel();

    spawn_local(async move {
        let cancellation = sender.cancellation();
        pin_mut!(future, cancellation);

        if let Either::Left((result, _)) = select(future, cancellation).await {
            sender.send(result).ok();
        }
    });

    receiver.map_err(|_err| TaskCancelled)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_tests {
    #[test]
    fn supplied_runtime_does_not_need_to_be_entered_when_spawning() {
        let runtime = tokio::runtime::Builder::new_current_thread() // NOLINT: the test must supply a runtime to the handle under test
            .build()
            .unwrap();
        let handle = super::AsyncRuntimeHandle::new_native(runtime.handle().clone());
        let (sender, receiver) = tokio::sync::oneshot::channel();

        handle.spawn_future(async move {
            sender.send(42).ok();
        });

        assert_eq!(runtime.block_on(receiver), Ok(42));
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod web_tests {
    use futures::channel::oneshot;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn returns_result() {
        assert_eq!(super::spawn_local_with_result(async { 42 }).await, Ok(42));
    }

    #[wasm_bindgen_test]
    async fn sleeps() {
        super::sleep(std::time::Duration::ZERO).await;
    }

    #[wasm_bindgen_test]
    async fn yield_now_allows_other_tasks_to_run() {
        let (other_task_tx, mut other_task_rx) = oneshot::channel();
        let (result_tx, result_rx) = oneshot::channel();

        // We defer this to later, so that we can first observe the second task.
        super::spawn_local(async move {
            super::yield_now().await;
            result_tx.send(other_task_rx.try_recv()).ok();
        });
        super::spawn_local(async move {
            other_task_tx.send(()).ok();
        });

        assert_eq!(result_rx.await, Ok(Ok(Some(()))));
    }

    #[wasm_bindgen_test]
    async fn dropping_result_cancels_spawned_future() {
        struct NotifyOnDrop(Option<oneshot::Sender<()>>);

        impl Drop for NotifyOnDrop {
            fn drop(&mut self) {
                self.0
                    .take()
                    .expect("drop notification should be sent once")
                    .send(())
                    .ok();
            }
        }

        let (started_tx, started_rx) = oneshot::channel();
        let (cancelled_tx, cancelled_rx) = oneshot::channel();
        let task = super::spawn_local_with_result(async move {
            let _notify_on_drop = NotifyOnDrop(Some(cancelled_tx));
            started_tx.send(()).ok();
            futures::future::pending::<()>().await;
        });

        started_rx.await.expect("spawned future should start");
        drop(task);
        cancelled_rx
            .await
            .expect("dropping the result should cancel the spawned future");
    }
}
