//! This build script collects all of our documentation and examples, and
//! uploads it to a Meilisearch instance for indexing.

mod ingest;
mod meili;

use std::io::stdout;
use std::io::Write as _;
use std::thread;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let documents = ingest::run()?;
    let search = meili::index(&documents).await?;

    let mut lines = Lines::spawn()?;
    loop {
        stdout().write_all(b"\n> ").unwrap();
        stdout().flush().unwrap();

        select! {
            _ = tokio::signal::ctrl_c() => {
                break
            }
            line = lines.next() => {
                let Some(line) = line else {
                    break
                };
                let line = line.as_str().trim();
                match line {
                    "quit" | "q" | "" => break,
                    _ => {
                        for result in search.query(line).await? {
                            println!("- {} [{}]", result.title(), result.url());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

struct Lines {
    shutdown_tx: mpsc::Sender<()>,
    data_rx: mpsc::Receiver<String>,
    _join_handle: thread::JoinHandle<anyhow::Result<()>>,
}

impl Lines {
    fn spawn() -> anyhow::Result<Self> {
        #![allow(clippy::significant_drop_tightening)]

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        let (data_tx, data_rx) = mpsc::channel(32);
        let join_handle = thread::Builder::new().name("stdin".into()).spawn(move || {
            let mut stdin = std::io::stdin().lines();
            loop {
                if let Ok(_) | Err(TryRecvError::Disconnected) = shutdown_rx.try_recv() {
                    break Ok(());
                }

                if let Some(line) = stdin.next().transpose()? {
                    data_tx
                        .blocking_send(line)
                        .map_err(|e| anyhow::anyhow!("failed to send line: {e:?}"))?;
                }
            }
        })?;

        Ok(Self {
            shutdown_tx,
            data_rx,
            _join_handle: join_handle,
        })
    }

    async fn next(&mut self) -> Option<String> {
        self.data_rx.recv().await
    }
}

impl Drop for Lines {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.try_send(());
    }
}
