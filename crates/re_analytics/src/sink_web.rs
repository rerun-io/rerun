// TODO(cmc): create a prod project and update this key.
// TODO: dedup
const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_XD1QbqTGdPJbzdVCbvbA9zGOG38wJFTl8RAwqMwBvTY";

#[derive(thiserror::Error, Debug)]
pub enum SinkError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[derive(Default, Debug, Clone)]
pub struct PostHogSink {}
