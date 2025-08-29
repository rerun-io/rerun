//! Simple benchmark suite for logging data.
//! The goal is to get an estimate for the entire process of logging data,
//! including serialization and processing by the recording stream.
//!
//! Timings are printed out while running, it's recommended to measure process run time to ensure
//! we account for all startup overheads and have all background threads finish.
//!
//! If not specified otherwise, memory recordings are used.
//!
//! The data we generate for benchmarking should be:
//! * minimal overhead to generate
//! * not homogeneous (arrow, ourselves, or even the compiler might exploit this)
//! * not trivially optimized out
//! * not random between runs
//!
//!
//! Run specific benchmark:
//! ```
//! cargo run -p log_benchmark --release -- images
//! ```
//!
//! For better whole-executable timing capture you can also first build the executable and then run:
//! ```
//! cargo build -p log_benchmark --release
//! ./target/release/log_benchmark images
//! ```
//!

use clap::Parser as _;
use rerun::external::re_log;

use crate::image::ImageCommand;

mod boxes3d_batch;
mod image;
mod points3d_large_batch;
mod points3d_many_individual;
mod points3d_shared;
mod scalars;

// ---

/// Very simple linear congruency "random" number generator to spread out values a bit.
pub fn lcg(lcg_state: &mut i64) -> i64 {
    *lcg_state = (1140671485_i64
        .wrapping_mul(*lcg_state)
        .wrapping_add(128201163))
        % 16777216;
    *lcg_state
}
// ---

#[derive(Debug, Clone, clap::Subcommand)]
enum Benchmark {
    #[command(name = "scalars")]
    Scalars,

    #[command(name = "points3d_large_batch")]
    Points3DLargeBatch,

    #[command(name = "points3d_many_individual")]
    Points3DManyIndividual,

    #[command(name = "boxes3d")]
    Boxes3D,

    #[command(name = "image")]
    Image(ImageCommand),
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// Which test should we run?.
    #[command(subcommand)]
    benchmark: Benchmark,

    /// If enabled, brings up the puffin profiler on startup.
    #[clap(long, default_value = "false")]
    profile: bool,

    /// If true, connect to a running Rerun viewer
    /// instead of writing to a memory buffer.
    #[clap(long, default_value = "false")]
    connect: bool,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    #[cfg(debug_assertions)]
    println!("WARNING: Debug build, timings will be inaccurate!");

    let Args {
        benchmark,
        profile,
        connect,
    } = Args::parse();

    // Start profiler first thing:
    let mut profiler = re_tracing::Profiler::default();
    if profile {
        profiler.start();
    }

    let (rec, _storage) = if connect {
        let rec = rerun::RecordingStreamBuilder::new("rerun_example_benchmark").connect_grpc()?;
        (rec, None)
    } else {
        let (rec, storage) =
            rerun::RecordingStreamBuilder::new("rerun_example_benchmark").memory()?;
        (rec, Some(storage))
    };

    println!("Running benchmark: {benchmark:?}");

    match benchmark {
        Benchmark::Scalars => scalars::run(&rec)?,
        Benchmark::Points3DLargeBatch => points3d_large_batch::run(&rec)?,
        Benchmark::Points3DManyIndividual => points3d_many_individual::run(&rec)?,
        Benchmark::Boxes3D => boxes3d_batch::run(&rec)?,
        Benchmark::Image(cmd) => cmd.run(&rec)?,
    }

    rec.flush_blocking()?;

    Ok(())
}
