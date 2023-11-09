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
//! Run all benchmarks:
//! ```
//! cargo run -p log_benchmark --release
//! ```
//!
//! Run specific benchmark:
//! ```
//! cargo run -p log_benchmark --release -- --benchmarks points3d_large_batch
//! ```
//!
//! For better whole-executable timing capture you can also first build the executable and then run:
//! ```
//! cargo build -p log_benchmark --release
//! ./target/release/log_benchmark
//! ```
//!

use clap::{Parser as _, ValueEnum as _};

mod image;
mod points3d_large_batch;
mod points3d_many_individual;
mod points3d_shared;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
enum Benchmark {
    #[value(name("points3d_large_batch"))]
    Points3DLargeBatch,

    #[value(name("points3d_many_individual"))]
    Points3DManyIndividual,

    #[value(name("image"))]
    Image,
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// Which test should we run? All of them by default.
    #[clap(long, value_enum)]
    benchmarks: Option<Vec<Benchmark>>,

    /// If enabled, brings up the puffin profiler on startup.
    #[clap(long, default_value = "false")]
    profile: bool,
}

fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    println!("WARNING: Debug build, timings will be inaccurate!");

    let args = Args::parse();

    let mut profiler = re_tracing::Profiler::default();
    if args.profile {
        profiler.start();
    }

    let benchmarks: Vec<Benchmark> = args.benchmarks.as_ref().map_or_else(
        || Benchmark::value_variants().to_vec(),
        |benchmarks| benchmarks.clone(),
    );

    for benchmark in benchmarks {
        println!("Running benchmark: {benchmark:?}");

        match benchmark {
            Benchmark::Points3DLargeBatch => points3d_large_batch::run()?,
            Benchmark::Points3DManyIndividual => points3d_many_individual::run()?,
            Benchmark::Image => image::run()?,
        }
    }

    Ok(())
}
