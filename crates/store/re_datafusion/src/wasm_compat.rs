use datafusion::common::DataFusionError;

/// This is a no-op on non-Wasm target, because the `tonic` future are already `Send`. See wasm
/// version for information.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub async fn make_future_send<F, T>(f: F) -> Result<T, DataFusionError>
where
    F: std::future::Future<Output = Result<T, DataFusionError>> + Send + 'static,
    T: Send + 'static,
{
    f.await
}

/// Convert a non-`Send` future into a `Send` one by spawning it and awaiting its result via a
/// channel.
///
/// This is required because `tonic` provides non-`Send` futures while `DataFusion` requires `Send`
/// ones.
#[cfg(target_arch = "wasm32")]
pub fn make_future_send<F, T>(
    f: F,
) -> impl std::future::Future<Output = Result<T, DataFusionError>> + Send + 'static
where
    F: std::future::Future<Output = Result<T, DataFusionError>> + 'static,
    T: Send + 'static,
{
    use futures::{FutureExt as _, pin_mut};
    use futures_util::future::{Either, select};

    let (mut tx, rx) = futures::channel::oneshot::channel();

    wasm_bindgen_futures::spawn_local(async {
        let cancellation = tx.cancellation();

        // needed by `select`
        pin_mut!(f, cancellation);

        match select(f, cancellation).await {
            Either::Left((result, _)) => {
                let _ = tx.send(result);
            }

            Either::Right(_) => {
                // If cancellation is triggered, it means that the future holding on `rx` was
                // dropped. So we don't need to do anything.
            }
        }
    });

    rx.map(|result| result.unwrap_or_else(|err| Err(DataFusionError::External(err.into()))))
}
