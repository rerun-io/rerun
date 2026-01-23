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

use crate::image::ImageCommand;
use crate::transform3d::Transform3DCommand;

mod boxes3d_batch;
mod image;
mod points3d_large_batch;
mod points3d_many_individual;
mod points3d_shared;
mod scalars;
mod transform3d;
mod very_large_chunk;

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

    #[command(name = "transform3d")]
    Transform3D(Transform3DCommand),

    #[command(name = "very_large_chunk")]
    VeryLargeChunk,
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

    /// If true, connect to a running Rerun viewer instead of writing to a memory buffer.
    #[clap(long, default_value = "false")]
    connect: bool,

    /// If true, perform an encode/decode roundtrip on the logged data.
    #[clap(long, default_value = "false")]
    check: bool,
}

fn main() -> anyhow::Result<()> {
    rerun::external::re_log::setup_logging();

    #[cfg(debug_assertions)]
    println!("WARNING: Debug build, timings will be inaccurate!");

    let Args {
        benchmark,
        profile,
        connect,
        check,
    } = Args::parse();

    // Start profiler first thing:
    let mut profiler = re_tracing::Profiler::default();
    if profile {
        profiler.start();
    }

    let (rec, storage) = if connect {
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
        Benchmark::Transform3D(cmd) => cmd.run(&rec)?,
        Benchmark::VeryLargeChunk => very_large_chunk::run(&rec)?,
    }

    rec.flush_blocking()?;

    // Being able to log fast isn't particularly useful if the data happens to be corrupt at the
    // other end, so make sure we can encode/decode everything that was logged.
    if check && let Some(storage) = storage {
        use rerun::external::re_log_encoding;
        use rerun::external::re_log_encoding::ToTransport as _;
        let msgs: anyhow::Result<Vec<_>> = storage
            .take()
            .into_iter()
            .map(|msg| Ok(msg.to_transport(re_log_encoding::rrd::Compression::LZ4)?))
            .collect();

        use rerun::external::re_log_encoding::ToApplication as _;
        let mut app_id_injector = re_log_encoding::DummyApplicationIdInjector::new("dummy");
        let msgs: anyhow::Result<Vec<_>> = msgs?
            .into_iter()
            .map(|msg| Ok(msg.to_application((&mut app_id_injector, None))?))
            .collect();

        let _ = msgs?;
    }

    Ok(())
}
