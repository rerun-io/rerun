//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p node_link_graph -- --connect
//! ```

use rerun::external::{log, re_log};
use strum::IntoEnumIterator;

mod examples;

#[derive(Copy, Clone, Debug, clap::ValueEnum, strum_macros::EnumIter)]
enum Example {
    Simple,
    Social,
    Disjoint,
    Lattice,
}

impl Example {
    fn run(&self, args: &Args) -> anyhow::Result<()> {
        match self {
            Example::Simple => examples::simple::run(args),
            Example::Social => examples::social::run(args),
            Example::Disjoint => examples::disjoint::run(args, 20),
            Example::Lattice => examples::lattice::run(args, 10),
        }
    }
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[arg(short, long)]
    example: Option<Example>,

    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    if let Some(example) = args.example {
        log::info!("Running example: {:?}", example);
        example.run(&args)?;
        return Ok(());
    }

    // By default we log all examples.
    for example in Example::iter() {
        log::info!("Running example: {:?}", example);
        example.run(&args)?;
    }

    Ok(())
}
