//! Browser task execution.

use std::future::Future;

use futures::TryFutureExt as _;

/// Indicates that a spawned task was canceled before producing its result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("browser task was canceled")]
pub struct TaskCancelled;

/// Spawns a future on the browser's local executor.
#[inline]
pub fn spawn_local(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

/// Spawns a possibly non-`Send` future and returns a `Send` future for its result.
///
/// Dropping the returned future cancels the spawned future.
pub fn spawn_local_with_result<F, T>(
    future: F,
) -> impl Future<Output = Result<T, TaskCancelled>> + Send + 'static
where
    F: Future<Output = T> + 'static,
    T: Send + 'static,
{
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

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn returns_result() {
        assert_eq!(super::spawn_local_with_result(async { 42 }).await, Ok(42));
    }

    #[wasm_bindgen_test]
    async fn dropping_result_cancels_task() {
        struct DropSignal(Option<futures::channel::oneshot::Sender<()>>);

        impl Drop for DropSignal {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    sender.send(()).ok();
                }
            }
        }

        let (sender, receiver) = futures::channel::oneshot::channel();
        let drop_signal = DropSignal(Some(sender));
        let task = super::spawn_local_with_result(async move {
            let _drop_signal = drop_signal;
            futures::future::pending::<()>().await;
        });

        drop(task);

        receiver.await.expect("spawned task was not canceled");
    }
}
