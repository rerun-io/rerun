#[derive(thiserror::Error, Debug)]
pub enum SinkError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[derive(Default, Debug, Clone)]
pub struct PostHogSink {}
