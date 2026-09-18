//! Optimize an RRD file with [`re_chunk_optimizer`] and write the result to another RRD file.
//!
//! An LLM-friendly benchmarking and profiling harness.
//!
//! ```text
//! cargo run --release -p re_chunk_optimizer --example optimize_rrd -- --help
//! ```
//!
//! Settings default to `OptimizationProfile::OBJECT_STORE`; no own-chunk rules are applied.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write as _;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use futures::StreamExt as _;
use parking_lot::Mutex;

use re_chunk_optimizer::{MergeSplitSettings, OptimizationSettings, optimize};
use re_chunk_store::OptimizationProfile;
use re_log_encoding::{Encoder, EncodingOptions, RrdChunkProvider, read_rrd_footer};
use re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::TimelineName;
use re_tracing::reexports::puffin;

/// Output chunks between two memory reports while streaming.
const CHUNKS_PER_MEM_REPORT: usize = 2000;

/// Output chunks between two puffin frames: bounds the size of the in-flight scope streams.
const CHUNKS_PER_PROFILE_FRAME: usize = 256;

/// Optimize an RRD file and write the result to another RRD file.
#[derive(clap::Parser)]
struct Args {
    /// Input RRD file.
    input: PathBuf,

    /// Output RRD file.
    #[arg(required_unless_present = "no_write")]
    output: Option<PathBuf>,

    /// Maximum chunk size in bytes; defaults to the `OBJECT_STORE` profile's value.
    #[arg(long)]
    max_bytes: Option<u64>,

    /// Timeline to sort the output chunks along.
    #[arg(long)]
    timeline: Option<String>,

    /// Plan and stream the optimized chunks without writing them out.
    #[arg(long)]
    dry_run: bool,

    /// Turn the `puffin` scopes on and print, at the end, the time spent per scope,
    /// sorted by self time (the scope's time minus that of its child scopes).
    #[arg(long)]
    profile: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let profile_stats = args.profile.then(ProfileStats::start);
    let mem = MemReport::start();

    let profile = OptimizationProfile::OBJECT_STORE;
    let max_bytes = args.max_bytes.unwrap_or(profile.chunk_max_bytes);
    let settings = OptimizationSettings {
        merge_split: NonZeroU64::new(max_bytes).map(|max_bytes| MergeSplitSettings {
            max_bytes,
            max_rows: NonZeroU64::new(profile.chunk_max_rows),
            max_rows_if_unsorted: NonZeroU64::new(profile.chunk_max_rows_if_unsorted),
        }),
        target_timeline: args
            .timeline
            .as_deref()
            .map(TimelineName::try_new)
            .transpose()?,
        own_chunk: Vec::new(),
    };

    let t_start = Instant::now();

    let reader = File::open(&args.input)?;
    let footer = futures::executor::block_on(read_rrd_footer(&reader))?
        .ok_or_else(|| anyhow::anyhow!("{}: no footer", args.input.display()))?;
    let t_footer = t_start.elapsed();
    eprintln!(
        "footer read: {t_footer:.2?} ({} store(s))",
        footer.manifests.len()
    );
    mem.report("after footer read");

    let mut encoder = if let Some(output) = args.output.filter(|_| !args.dry_run) {
        let file = std::io::BufWriter::new(File::create(output)?);
        Some(Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            file,
        )?)
    } else {
        None
    };

    let mut num_chunks_in = 0_usize;
    let mut num_chunks_out = 0_usize;
    let mut num_rows_out = 0_u64;
    let mut t_plan = Duration::ZERO;
    let mut t_stream = Duration::ZERO;
    let mut t_encode = Duration::ZERO;

    let mut store_ids: Vec<_> = footer.manifests.keys().cloned().collect();
    store_ids.sort();
    for store_id in store_ids {
        if store_id.is_blueprint() {
            eprintln!("skipping blueprint store {store_id}");
            continue;
        }

        let raw = Arc::new(footer.manifests[&store_id].clone());
        num_chunks_in += raw.data.num_rows();

        let provider = Arc::new(RrdChunkProvider::from_reader(
            File::open(&args.input)?,
            args.input.display().to_string(),
            raw,
        )?);
        mem.report("after provider (RrdManifest::try_new)");

        if let Some(encoder) = &mut encoder {
            encoder.append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: re_log_types::external::re_tuid::Tuid::new(),
                info: StoreInfo::new(store_id.clone(), StoreSource::Other("optimize_rrd".into())),
            }))?;
        }

        let mut stream = std::pin::pin!(timed(&mut t_plan, || optimize(provider, &settings))?);
        mem.report("after optimize() (view + plan)");
        puffin::GlobalProfiler::lock().new_frame();

        futures::executor::block_on(async {
            loop {
                let next_start = Instant::now();
                let Some(chunk) = stream.next().await else {
                    break;
                };
                let chunk = chunk?;
                t_stream += next_start.elapsed();

                num_chunks_out += 1;
                num_rows_out += chunk.num_rows() as u64;

                if let Some(encoder) = &mut encoder {
                    re_tracing::profile_scope!("encode");
                    timed(&mut t_encode, || -> anyhow::Result<()> {
                        let arrow_msg = chunk.to_arrow_msg()?;
                        encoder.append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg))?;
                        Ok(())
                    })?;
                }

                if num_chunks_out.is_multiple_of(CHUNKS_PER_PROFILE_FRAME) {
                    puffin::GlobalProfiler::lock().new_frame();
                }
                if num_chunks_out.is_multiple_of(CHUNKS_PER_MEM_REPORT) {
                    mem.report(&format!("streaming, {num_chunks_out} chunks out"));
                }
            }
            anyhow::Ok(())
        })?;
    }

    if let Some(mut encoder) = encoder {
        timed(&mut t_encode, || -> anyhow::Result<()> {
            encoder.finish()?;
            encoder.into_inner()?.flush()?;
            Ok(())
        })?;
    }
    puffin::GlobalProfiler::lock().new_frame();

    let t_total = t_start.elapsed();
    eprintln!("plan (view + planner): {t_plan:.2?}");
    eprintln!("stream (load + merge/split): {t_stream:.2?}");
    eprintln!("encode + write: {t_encode:.2?}");
    eprintln!("total: {t_total:.2?}");
    eprintln!("chunks: {num_chunks_in} -> {num_chunks_out} ({num_rows_out} rows out)");
    mem.report("end");

    if let Some(stats) = profile_stats {
        stats.print(t_total);
    }

    Ok(())
}

/// Runs `f` and adds its wall-clock duration to `acc`.
fn timed<T>(acc: &mut Duration, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    *acc += start.elapsed();
    result
}

// --- Memory ---

/// Prints resident and allocator-counted bytes at phase boundaries, plus the peak resident size
/// seen by a sampling thread since start.
struct MemReport {
    peak_resident: Arc<std::sync::atomic::AtomicU64>,
    baseline: re_memory::MemoryUse,
}

impl MemReport {
    fn start() -> Self {
        let peak_resident = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let peak = peak_resident.clone();
        std::thread::Builder::new()
            .name("mem-sampler".to_owned())
            .spawn(move || sample_peak_resident(&peak))
            .expect("failed to spawn the memory sampler thread");
        Self {
            peak_resident,
            baseline: re_memory::MemoryUse::capture(),
        }
    }

    fn report(&self, label: &str) {
        let now = re_memory::MemoryUse::capture();
        if let Some(resident) = now.resident {
            self.peak_resident
                .fetch_max(resident, std::sync::atomic::Ordering::Relaxed);
        }
        let gib = |b: Option<u64>| b.map_or(-1.0, |b| b as f64 / (1024.0 * 1024.0 * 1024.0));
        eprintln!(
            "[mem] rss {:>6.2} GiB  peak rss {:>6.2} GiB  ({label})", // NOLINT: double spaces on purpose
            gib(now.resident),
            gib(Some(
                self.peak_resident
                    .load(std::sync::atomic::Ordering::Relaxed)
            )),
        );
        let _ = self.baseline;
    }
}

/// Samples the resident size every 10 ms into `peak`, for the life of the process.
fn sample_peak_resident(peak: &std::sync::atomic::AtomicU64) -> ! {
    loop {
        if let Some(resident) = re_memory::MemoryUse::capture().resident {
            peak.fetch_max(resident, std::sync::atomic::Ordering::Relaxed);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

// --- Profiling ---

#[derive(Default, Clone, Copy)]
struct ScopeStats {
    count: u64,
    inclusive_ns: i64,
    exclusive_ns: i64,
}

/// Aggregates every puffin scope of every frame by scope, as the frames are produced.
struct ProfileStats {
    inner: Arc<Mutex<ProfileStatsInner>>,
    sink_id: puffin::FrameSinkId,
}

#[derive(Default)]
struct ProfileStatsInner {
    scopes: puffin::ScopeCollection,
    stats: HashMap<puffin::ScopeId, ScopeStats>,
}

impl ProfileStats {
    fn start() -> Self {
        puffin::set_scopes_on(true);
        let inner = Arc::new(Mutex::new(ProfileStatsInner::default()));
        let sink_inner = inner.clone();
        let sink_id = puffin::GlobalProfiler::lock().add_sink(Box::new(move |frame| {
            let mut inner = sink_inner.lock();
            inner.ingest(&frame);
        }));
        Self { inner, sink_id }
    }

    fn print(self, wall: std::time::Duration) {
        puffin::GlobalProfiler::lock().remove_sink(self.sink_id);
        let inner = self.inner.lock();

        let mut rows: Vec<(String, ScopeStats)> = inner
            .stats
            .iter()
            .map(|(id, stats)| {
                let name = inner.scopes.fetch_by_id(id).map_or_else(
                    || format!("<scope {}>", id.0),
                    |details| {
                        let name = details
                            .scope_name
                            .as_deref()
                            .unwrap_or(&details.function_name);
                        format!("{name}  ({}:{})", details.file_path, details.line_nr)
                    },
                );
                (name, *stats)
            })
            .collect();
        rows.sort_by_key(|(_, stats)| std::cmp::Reverse(stats.exclusive_ns));

        let wall_ns = wall.as_nanos() as f64;
        let scoped_ns: i64 = rows.iter().map(|(_, s)| s.exclusive_ns).sum();
        eprintln!();
        eprintln!(
            "{:>10} {:>10} {:>7} {:>10} {:>7} {:>11}  scope",
            "self", "self%", "total", "total%", "count", "self/call"
        );
        for (name, stats) in &rows {
            eprintln!(
                "{:>9.2}s {:>9.1}% {:>6.2}s {:>9.1}% {:>7} {:>9.2}us  {name}",
                stats.exclusive_ns as f64 / 1e9,
                stats.exclusive_ns as f64 / wall_ns * 100.0,
                stats.inclusive_ns as f64 / 1e9,
                stats.inclusive_ns as f64 / wall_ns * 100.0,
                stats.count,
                stats.exclusive_ns as f64 / 1e3 / stats.count.max(1) as f64,
            );
        }
        eprintln!(
            "{:>9.2}s {:>9.1}%  unscoped (wall {:.2}s)",
            (wall_ns - scoped_ns as f64) / 1e9,
            (wall_ns - scoped_ns as f64) / wall_ns * 100.0,
            wall_ns / 1e9,
        );
    }
}

impl ProfileStatsInner {
    fn ingest(&mut self, frame: &puffin::FrameData) {
        for details in &frame.scope_delta {
            self.scopes.insert(details.clone());
        }
        let Some(unpacked) = frame.unpacked().ok() else {
            return;
        };
        for stream_info in unpacked.thread_streams.values() {
            let reader = puffin::Reader::from_start(&stream_info.stream);
            self.walk(&stream_info.stream, reader);
        }
    }

    /// Accumulate the scopes `reader` yields and their descendants; returns their summed time.
    fn walk(&mut self, stream: &puffin::Stream, reader: puffin::Reader<'_>) -> i64 {
        let mut sum_ns = 0;
        for scope in reader {
            let Ok(scope) = scope else {
                break;
            };
            let children_ns = match puffin::Reader::with_offset(stream, scope.child_begin_position)
            {
                Ok(children) => self.walk(stream, children),
                Err(_) => 0,
            };
            let duration_ns = scope.record.duration_ns;
            let stats = self.stats.entry(scope.id).or_default();
            stats.count += 1;
            stats.inclusive_ns += duration_ns;
            stats.exclusive_ns += duration_ns - children_ns;
            sum_ns += duration_ns;
        }
        sum_ns
    }
}
