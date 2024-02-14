//! This build script collects all of our documentation and examples, and
//! uploads it to a Meilisearch instance for indexing.

mod build;
mod ingest;
mod meili;
mod repl;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let Some(cmd) = args.cmd else {
        let help_text = <Args as argh::FromArgs>::from_args(&["search-index"], &["--help"])
            .err()
            .unwrap()
            .output;

        eprintln!("{help_text}");
        return Ok(());
    };

    match cmd {
        Cmd::Repl(cmd) => cmd.run().await,
        Cmd::Build(cmd) => cmd.run().await,
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
    Build(build::Build),
}

const DEFAULT_URL: &str = "http://localhost:7700";
const DEFAULT_KEY: &str = "test";
const DEFAULT_INDEX: &str = "temp";
