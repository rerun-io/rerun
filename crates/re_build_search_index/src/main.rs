//! This build script collects all of our documentation and examples, and
//! uploads it to a Meilisearch instance for indexing.

mod index;
mod ingest;
mod meili;
mod repl;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    match args.cmd.unwrap_or_default() {
        Cmd::Repl(cmd) => cmd.run().await,
        Cmd::Index(cmd) => cmd.run().await,
    }
}

/// Meilisearch indexer and REPL
#[derive(argh::FromArgs)]
struct Args {
    #[argh(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Cmd {
    Repl(repl::Repl),
    Index(index::Index),
}

impl Default for Cmd {
    fn default() -> Self {
        Self::Index(Default::default())
    }
}

const DEFAULT_URL: &str = "http://localhost:7700";
const DEFAULT_KEY: &str = "test";
const DEFAULT_INDEX: &str = "temp";
