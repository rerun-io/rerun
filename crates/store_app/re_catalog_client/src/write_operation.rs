use std::sync::{Arc, OnceLock};
use std::time::Duration;

use re_async::WasmNotSend;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_redap_client::ApiError;
use tokio::sync::watch;
use tracing::Instrument as _;

use crate::{CatalogClient, RegistrationError, WriteError, WriteHandle};

#[derive(Clone, Debug)]
enum WriteState {
    Uploading,
    Registering(WriteHandle),
    Failed(Arc<WriteError>),
}

// TODO(RR-6038): Expose upload progress and richer registration status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteStatus {
    Uploading,
    Registering,
    Succeeded,
    Failed,
}

/// The uploaded object's URL is retained even if no new segments were registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteResult {
    pub segment_ids: Vec<SegmentId>,
    pub storage_url: url::Url,
}

#[derive(Debug, thiserror::Error)]
pub enum WriteOperationError {
    #[error("write failed")]
    Write(#[source] Arc<WriteError>),

    /// Terminal: waiting again returns the same error.
    #[error("registration failed")]
    RegistrationFailed(#[source] ApiError),

    /// Waiting again may succeed.
    #[error("failed to query registration status")]
    RegistrationQuery(#[source] ApiError),

    #[error("write worker stopped before reporting an outcome; remote outcome is unknown")]
    WorkerStopped,
}

/// A background upload followed by registration.
///
/// Dropping this operation or a wait future does not stop the upload or cancel server work.
/// Registration is queried on demand; only terminal outcomes are cached.
#[derive(Debug)]
pub struct WriteOperation {
    state: watch::Receiver<WriteState>,
    registration: OnceLock<Result<WriteResult, ApiError>>,
}

impl CatalogClient {
    /// Start an upload and registration submission using a clone of this client.
    ///
    /// The upload runs on the client's runtime even if the returned operation is dropped.
    /// Use the ordinary async write methods instead when dropping the future should stop the upload.
    pub fn start_write<F>(&self, upload: impl FnOnce(Self) -> F) -> WriteOperation
    where
        F: Future<Output = Result<WriteHandle, Box<WriteError>>> + WasmNotSend + 'static,
    {
        let (sender, state) = watch::channel(WriteState::Uploading);
        let upload = upload(self.clone());
        self.runtime.spawn_future(
            async move {
                sender.send_replace(match upload.await {
                    Ok(write) => WriteState::Registering(write),
                    Err(err) => WriteState::Failed(Arc::from(err)),
                });
            }
            .in_current_span(),
        );
        WriteOperation {
            state,
            registration: OnceLock::new(),
        }
    }
}

impl WriteOperation {
    /// The status of the upload and registration.
    ///
    /// This does not query the server.
    /// It reports [`WriteStatus::Succeeded`], or [`WriteStatus::Failed`] for a registration failure,
    /// only after [`Self::wait`] has observed that outcome.
    /// Until then, it reports [`WriteStatus::Registering`].
    pub fn status(&self) -> WriteStatus {
        let Self {
            state,
            registration,
        } = self;
        match registration.get() {
            Some(Ok(_)) => return WriteStatus::Succeeded,
            Some(Err(_)) => return WriteStatus::Failed,
            None => {}
        }
        match &*state.borrow() {
            WriteState::Registering(_) => WriteStatus::Registering,
            WriteState::Failed(_) => WriteStatus::Failed,
            WriteState::Uploading if state.has_changed().is_err() => WriteStatus::Failed,
            WriteState::Uploading => WriteStatus::Uploading,
        }
    }

    /// Wait for the upload and all its registration tasks to complete.
    ///
    /// `timeout` applies only to this waiter's registration query, not the upload.
    /// Query failures and timeouts leave the operation retryable.
    /// Concurrent waiters issue independent registration queries.
    pub async fn wait(&self, timeout: Duration) -> Result<WriteResult, WriteOperationError> {
        let Self {
            state,
            registration,
        } = self;
        let mut state = state.clone();
        loop {
            if let Some(result) = registration.get() {
                return result
                    .clone()
                    .map_err(WriteOperationError::RegistrationFailed);
            }
            let current = state.borrow_and_update().clone();
            match current {
                WriteState::Uploading => {}
                WriteState::Registering(write) => {
                    let result = match write.wait(timeout).await {
                        Ok(segment_ids) => Ok(WriteResult {
                            segment_ids,
                            storage_url: write.storage_url,
                        }),
                        Err(RegistrationError::Query(err)) => {
                            return Err(WriteOperationError::RegistrationQuery(err));
                        }
                        Err(RegistrationError::Failed(err)) => Err(err),
                    };
                    return registration
                        .get_or_init(|| result)
                        .clone()
                        .map_err(WriteOperationError::RegistrationFailed);
                }
                WriteState::Failed(err) => return Err(WriteOperationError::Write(err)),
            }
            state
                .changed()
                .await
                .map_err(|_err| WriteOperationError::WorkerStopped)?;
        }
    }
}
