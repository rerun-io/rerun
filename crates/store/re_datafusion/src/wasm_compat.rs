use re_redap_client::ApiResult;

/// This is a no-op on non-Wasm target, because the `tonic` future are already `Send`. See wasm
/// version for information.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub async fn make_future_send<F, T>(f: F) -> ApiResult<T>
where
    F: std::future::Future<Output = ApiResult<T>> + Send + 'static,
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
) -> impl std::future::Future<Output = ApiResult<T>> + Send + 'static
where
    F: std::future::Future<Output = ApiResult<T>> + 'static,
    T: Send + 'static,
{
    let task = re_async::spawn_local_with_result(f);

    async move {
        task.await.unwrap_or_else(|_cancelled| {
            Err(re_redap_client::ApiError::internal(
                "wasm task cancelled unexpectedly",
            ))
        })
    }
}
