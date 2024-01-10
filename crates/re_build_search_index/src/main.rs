//! This build script collects all of our documentation and examples, and
//! uploads it to a Meilisearch instance for indexing.

use camino::Utf8Path;
use std::cell::Cell;
use std::io::stdout;
use std::io::Write as _;
use std::path::Path;
use std::thread;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = meilisearch_sdk::Client::new("http://localhost:7700", Some("test"));

    let metadata = re_build_tools::cargo_metadata()?;

    let id_gen = IdGen::new();
    let mut documents = Vec::new();

    ingest_docs(&metadata.workspace_root, &id_gen, &mut documents)?;
    // ingest_rustdoc(&metadata.workspace_root, &id_gen, &mut documents)?;

    delete_index(&client).await?; // clean index from last run
    create_index(&client, &documents).await?;

    let mut lines = Lines::spawn()?;
    loop {
        stdout().write_all(b"> ").unwrap();
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
                        let results = do_search(&client, line).await?;
                        for hit in &results.hits {
                            println!("{}", hit.result.title);
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

const INDEX_KEY: &str = "temp";

async fn do_search(
    client: &meilisearch_sdk::Client,
    query: &str,
) -> anyhow::Result<meilisearch_sdk::SearchResults<Document>> {
    Ok(client
        .index(INDEX_KEY)
        .search()
        .with_query(query)
        .execute()
        .await?)
}

async fn index_exists(client: &meilisearch_sdk::Client) -> anyhow::Result<bool> {
    use meilisearch_sdk::{Error, ErrorCode, MeilisearchError};
    match client.get_index(INDEX_KEY).await {
        Ok(_) => Ok(true),
        Err(Error::Meilisearch(MeilisearchError {
            error_code: ErrorCode::IndexNotFound,
            ..
        })) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

async fn create_index(
    client: &meilisearch_sdk::Client,
    documents: &[Document],
) -> anyhow::Result<()> {
    client
        .index(INDEX_KEY)
        .add_or_update(documents, Some("id"))
        .await?
        .wait_for_completion(client, None, None)
        .await?
        .to_result()
}

async fn delete_index(client: &meilisearch_sdk::Client) -> anyhow::Result<()> {
    if !index_exists(client).await? {
        return Ok(());
    }

    client
        .delete_index(INDEX_KEY)
        .await?
        .wait_for_completion(client, None, None)
        .await?
        .to_result()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Document {
    id: u64,
    kind: Kind,
    title: String,
    tags: Vec<String>,
    content: String,
    url: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Docs,
    Examples,
    Rust,
    Python,
    Cpp,
}

fn ingest_docs(
    workspace_root: &Utf8Path,
    id_gen: &IdGen,
    documents: &mut Vec<Document>,
) -> anyhow::Result<()> {
    let dir = workspace_root.join("docs").join("content");
    for entry in glob::glob(&format!("{dir}/**/*.md"))? {
        let entry = entry?;
        let url = format!(
            "https://rerun.io/docs/{}",
            entry.strip_prefix(&dir)?.display()
        );
        let (frontmatter, body) = parse_docs_frontmatter(&entry)?;

        documents.push(Document {
            id: id_gen.next(),
            kind: Kind::Docs,
            title: frontmatter.title,
            tags: vec![],
            content: body,
            url,
        });
    }

    Ok(())
}

fn ingest_rustdoc(
    workspace_root: &Utf8Path,
    id_gen: &IdGen,
    documents: &mut Vec<Document>,
) -> anyhow::Result<()> {
    todo!()
}

struct IdGen {
    v: Cell<u64>,
}

impl IdGen {
    fn new() -> Self {
        Self { v: Cell::new(0) }
    }

    fn next(&self) -> u64 {
        self.v.replace(self.v.get() + 1)
    }
}

#[derive(serde::Deserialize)]
struct DocsFrontmatter {
    title: String,
}

fn parse_docs_frontmatter<P: AsRef<Path>>(path: P) -> anyhow::Result<(DocsFrontmatter, String)> {
    const START: &str = "---";
    const END: &str = "---";

    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;

    let Some(start) = content.find(START) else {
        anyhow::bail!("\"{}\" is missing frontmatter", path.display())
    };
    let start = start + START.len();

    let Some(end) = content[start..].find(END) else {
        anyhow::bail!(
            "\"{}\" has invalid frontmatter: missing {END:?} terminator",
            path.display()
        );
    };
    let end = start + end;

    let frontmatter: DocsFrontmatter =
        serde_yaml::from_str(content[start..end].trim()).map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse YAML metadata of {:?}: {err}",
                path.parent().unwrap().file_name().unwrap()
            )
        })?;

    Ok((frontmatter, content[end + END.len()..].trim().to_owned()))
}

trait ToResult {
    fn to_result(self) -> anyhow::Result<()>;
}

impl ToResult for meilisearch_sdk::Task {
    fn to_result(self) -> anyhow::Result<()> {
        if self.is_failure() {
            Err(self.unwrap_failure().into())
        } else {
            Ok(())
        }
    }
}
