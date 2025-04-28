use datafusion::common::DataFusionError;

#[cfg(target_arch = "wasm32")]
use futures::FutureExt as _;

/// This is a no-op on non-Wasm target. See wasm version for information.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub async fn wasm_wrapper<F, T>(f: F) -> Result<T, DataFusionError>
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
pub fn wasm_wrapper<F, T>(
    f: F,
) -> impl std::future::Future<Output = Result<T, DataFusionError>> + Send + 'static
where
    F: std::future::Future<Output = Result<T, DataFusionError>> + 'static,
    T: Send + 'static,
{
    let (tx, rx) = futures::channel::oneshot::channel();

    wasm_bindgen_futures::spawn_local(async {
        let _ = tx.send(f.await);
    });

    rx.then(|result| async {
        result.unwrap_or_else(|err| Err(DataFusionError::External(err.into())))
    })
}
