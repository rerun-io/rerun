use crate::meili::SearchClient;
use crate::{ingest, meili, DEFAULT_KEY, DEFAULT_URL};

use std::io::stdout;
use std::io::Write as _;
use std::ops::ControlFlow;
use std::thread;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

/// Simple terminal search client
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "repl")]
pub struct Repl {
    /// name of the meilisearch index to create/query
    #[argh(positional)]
    index_name: String,

    /// meilisearch URL
    #[argh(option, long = "url", default = "DEFAULT_URL.into()")]
    meilisearch_url: String,

    /// meilisearch master key (must support both read and write)
    #[argh(option, long = "master-key", default = "DEFAULT_KEY.into()")]
    meilisearch_master_key: String,
}

impl Repl {
    pub async fn run(self) -> anyhow::Result<()> {
        let client = meili::connect(&self.meilisearch_url, &self.meilisearch_master_key).await?;
        let documents = ingest::run()?;
        client.index(&self.index_name, &documents).await?;

        let mut lines = Lines::spawn()?;
        loop {
            stdout().write_all(b"\n> ").unwrap();
            stdout().flush().unwrap();

            select! {
                _ = tokio::signal::ctrl_c() => {
                    break Ok(())
                }
                line = lines.next() => {
                    let Some(line) = line else {
                        break Ok(())
                    };
                    if self.handle_line(&client, &line).await? == ControlFlow::Break(()) {
                        break Ok(())
                    }
                }
            }
        }
    }

    async fn handle_line(
        &self,
        search: &SearchClient,
        line: &str,
    ) -> anyhow::Result<ControlFlow<()>> {
        let line = line.trim();
        match line {
            "quit" | "q" | "" => return Ok(ControlFlow::Break(())),
            "reindex" => {
                let documents = ingest::run()?;
                search.index(&self.index_name, &documents).await?;
            }
            _ => {
                for result in search.query(&self.index_name, line, Some(4)).await? {
                    let content = result.content();
                    println!("### {} [{}]", result.title(), result.url(),);
                    if content.len() > 200 {
                        println!("{}…\n", &content[..200]);
                    } else {
                        println!("{content}\n");
                    }
                }
            }
        }
        Ok(ControlFlow::Continue(()))
    }
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
                        .map_err(|err| anyhow::anyhow!("failed to send line: {err}"))?;
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
