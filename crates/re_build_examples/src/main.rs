//! This build script collects all examples which should be part of our example page,
//! and either runs them to produce `.rrd` files, or builds a manifest file which
//! serves as an index for the files.
//!
//! It identifies runnable examples by checking if they have `channel` set in
//! their `README.md` frontmatter. The available values are:
//! - `main` for simple/fast examples built on each PR and the `main` branch
//! - `nightly` for heavier examples built once per day
//!
//! An example may also specify args to be run with via the frontmatter
//! `build_args` string array.

pub use re_build_examples::*;

use argh::FromArgs;

fn main() -> anyhow::Result<()> {
    re_build_tools::set_output_cargo_build_instructions(false);

    let args: Args = argh::from_env();
    match args.cmd {
        Cmd::Rrd(cmd) => cmd.run(),
        Cmd::Manifest(cmd) => cmd.run(),
        Cmd::Snippets(cmd) => cmd.run(),
    }
}

/// Build examples and their manifest.
#[derive(FromArgs)]
struct Args {
    #[argh(subcommand)]
    cmd: Cmd,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Cmd {
    Rrd(rrd::Rrd),
    Manifest(manifest::Manifest),
    Snippets(snippets::Snippets),
}
