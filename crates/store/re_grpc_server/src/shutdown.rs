use tokio::sync::oneshot;

pub fn shutdown() -> (Signal, Shutdown) {
    let (tx, rx) = oneshot::channel();
    (Signal(tx), Shutdown(Some(rx)))
}

pub fn never() -> Shutdown {
    Shutdown(None)
}

pub struct Signal(oneshot::Sender<()>);

impl Signal {
    pub fn stop(self) {
        self.0.send(()).ok();
    }
}

pub struct Shutdown(Option<oneshot::Receiver<()>>);

impl Shutdown {
    pub async fn wait(self) {
        if let Some(rx) = self.0 {
            rx.await.ok();
        } else {
            // Never resolve
            std::future::pending::<()>().await;
        }
    }
}
