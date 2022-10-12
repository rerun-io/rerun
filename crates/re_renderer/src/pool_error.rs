use thiserror::Error;

#[derive(Error, Debug)]
pub enum PoolError {
    #[error("Requested resource isn't available yet of the handle is no longer valid")]
    ResourceNotAvailable,
    #[error("The passed resource handle was null")]
    NullHandle,
}
