use futures::channel::oneshot;

/// Answers one request that the agent is blocked on.
///
/// Dropping it unanswered never unblocks the agent: the request then stays open until the
/// agent cancels it or the backend shuts down.
#[must_use]
pub struct Responder<T> {
    tx: oneshot::Sender<T>,
}

/// The agent stopped waiting for the answer, e.g. because it cancelled the request.
#[derive(Debug, thiserror::Error)]
#[error("the agent is no longer waiting for an answer")]
pub struct AgentStoppedWaiting;

impl<T> Responder<T> {
    /// A responder, and the receiver its answer arrives on.
    ///
    /// The backend keeps the receiver, and drops it once the agent stops waiting.
    pub fn channel() -> (Self, oneshot::Receiver<T>) {
        let (tx, rx) = oneshot::channel();
        (Self { tx }, rx)
    }

    pub fn respond(self, response: T) -> Result<(), AgentStoppedWaiting> {
        self.tx
            .send(response)
            .map_err(|_unsent_response| AgentStoppedWaiting)
    }

    /// Whether the agent stopped waiting, so that answering is pointless.
    pub fn is_cancelled(&self) -> bool {
        self.tx.is_canceled()
    }
}

impl<T> std::fmt::Debug for Responder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Responder")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}
