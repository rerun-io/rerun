//! This build script collects all of our documentation and examples, and
//! uploads it to a Meilisearch instance for indexing.

mod build;
mod ingest;
mod meili;
mod repl;
mod util;

/// Meilisearch indexer and REPL
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "search-index")]
pub struct Args {
    #[argh(subcommand)]
    cmd: Cmd,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Cmd {
    Repl(repl::Repl),
    Build(build::Build),
}

pub fn main(args: Args) -> anyhow::Result<()> {
    match args.cmd {
        Cmd::Repl(cmd) => cmd.run(),
        Cmd::Build(cmd) => cmd.run(),
    }
}

const DEFAULT_URL: &str = "http://localhost:7700";
const DEFAULT_KEY: &str = "test";
const DEFAULT_INDEX: &str = "temp";
