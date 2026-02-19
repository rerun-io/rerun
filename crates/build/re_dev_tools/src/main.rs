//! Crate that combines several development utilities.
//!
//! To get an overview over all tools run `pixi run dev-tools --help`.

// Exception for the build crates as it's not production code.
#![allow(clippy::panic)]

use argh::FromArgs;

mod build_examples;
mod build_search_index;
mod build_web_viewer;

#[derive(FromArgs)]
#[argh(subcommand)]
enum Commands {
    BuildExamples(build_examples::Args),
    BuildWebViewer(build_web_viewer::Args),
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
        Commands::BuildWebViewer(args) => build_web_viewer::main(args),
    }
}
