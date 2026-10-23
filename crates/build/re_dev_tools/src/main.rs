//! Crate that combines several development utilities.
//!
//! To get an overview over all tools run `pixi run dev-tools --help`.
use argh::FromArgs;

mod build_examples;
mod build_search_index;

#[derive(FromArgs)]
#[argh(subcommand)]
enum Commands {
    BuildExamples(build_examples::Args),
    SearchIndex(build_search_index::Args),
}

/// Various development tools for Rerun.
#[derive(FromArgs)]
struct TopLevel {
    #[argh(subcommand)]
    cmd: Commands,
}

fn main() -> anyhow::Result<()> {
    let args: TopLevel = argh::from_env();

    match args.cmd {
        Commands::BuildExamples(args) => build_examples::main(args),
        Commands::SearchIndex(args) => build_search_index::main(args),
    }
}
