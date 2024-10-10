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
}

impl Example {
    fn run(&self, args: &Args) -> anyhow::Result<()> {
        match self {
            Example::Simple => examples::simple::run(args),
            Example::Social => examples::social::run(args),
        }
    }
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[arg(short, long)]
    example: Option<Example>
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
