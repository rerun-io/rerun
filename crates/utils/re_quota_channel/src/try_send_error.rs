/// Error returned by [`crate::Sender::try_send`].
#[derive(Debug)]
pub enum TrySendError<T> {
    /// The channel's byte capacity is full.
    Full(T),

    /// The channel is disconnected.
    Disconnected(T),
}

impl<T> TrySendError<T> {
    /// Unwrap the message that couldn't be sent.
    pub fn into_inner(self) -> T {
        match self {
            Self::Full(msg) | Self::Disconnected(msg) => msg,
        }
    }

    /// Returns `true` if the channel is full.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full(_))
    }

    /// Returns `true` if the channel is disconnected.
    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected(_))
    }
}

impl<T> std::fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(_) => write!(f, "channel byte capacity exceeded"),
            Self::Disconnected(_) => write!(f, "channel disconnected"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for TrySendError<T> {}
