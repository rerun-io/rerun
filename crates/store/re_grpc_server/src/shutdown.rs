use parking_lot::Mutex;
use tokio::sync::oneshot;

pub fn shutdown() -> (Signal, Shutdown) {
    let (tx, rx) = oneshot::channel();
    (Signal(Mutex::new(Some(tx))), Shutdown(Some(rx)))
}

pub fn never() -> Shutdown {
    Shutdown(None)
}

pub struct Signal(Mutex<Option<oneshot::Sender<()>>>);

impl Signal {
    /// Ask the server to shut down.
    ///
    /// Subsequent calls to this function have no effect.
    pub fn stop(&self) {
        if let Some(sender) = self.0.lock().take() {
            sender.send(()).ok();
        }
    }
}

pub struct Shutdown(Option<oneshot::Receiver<()>>);

impl Shutdown {
    /// Returns a future that resolves when the signal is sent.
    ///
    /// If this was constructed with [`never()`], then it never resolves.
    pub async fn wait(self) {
        if let Some(rx) = self.0 {
            rx.await.ok();
        } else {
            // Never resolve
            std::future::pending::<()>().await;
        }
    }
}
