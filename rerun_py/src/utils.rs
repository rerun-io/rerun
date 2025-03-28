use std::{future::Future, sync::OnceLock};

use pyo3::Python;
use tokio::runtime::Runtime;

/// Utility to get the Tokio Runtime from Python
#[inline]
pub(crate) fn get_tokio_runtime() -> &'static Runtime {
    // NOTE: Other pyo3 python libraries have had issues with using tokio
    // behind a forking app-server like `gunicorn`
    // If we run into that problem, in the future we can look to `delta-rs`
    // which adds a check in that disallows calls from a forked process
    // https://github.com/delta-io/delta-rs/blob/87010461cfe01563d91a4b9cd6fa468e2ad5f283/python/src/utils.rs#L10-L31
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"))
}

pub fn wait_for_future<F>(py: Python<'_>, f: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    let runtime: &Runtime = get_tokio_runtime();
    py.allow_threads(|| runtime.block_on(f))
}
