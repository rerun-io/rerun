//! This build script collects all of our documentation and examples, and
//! uploads it to a Meilisearch instance for indexing.

mod index;
mod ingest;
mod meili;
mod repl;

use argh::FromArgs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    match args.cmd {
        Cmd::Repl(cmd) => cmd.run().await,
        Cmd::Index(cmd) => cmd.run().await,
    }
}

/// Meilisearch indexer and REPL
#[derive(FromArgs)]
struct Args {
    #[argh(subcommand)]
    cmd: Cmd,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Cmd {
    Repl(repl::Repl),
    Index(index::Index),
}

const DEFAULT_URL: &str = "http://localhost:7700";
const DEFAULT_KEY: &str = "test";
