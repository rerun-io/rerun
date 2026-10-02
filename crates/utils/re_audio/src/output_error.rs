/// Why the audio output device could not be opened, or stopped.
#[derive(thiserror::Error, Debug, Clone)]
pub enum OutputError {
    #[error("Failed to load the system audio library: {0}")]
    LibraryLoad(String),

    #[error("Audio device error: {0}")]
    Device(String),
}
