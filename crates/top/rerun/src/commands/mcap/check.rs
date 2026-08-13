//! `rerun mcap check` — check an MCAP file for structural and timeline issues.
//!
//! Each MCAP chunk holds messages from many topics interleaved together.
//! When loaded into Rerun, an MCAP chunk is split into one Rerun chunk per topic, and each Rerun
//! chunk is reordered using [`re_chunk::Chunk::from_auto_row_ids`].
//! A chunk's timelines remain sorted only if every timeline agrees on the row order.
//!
//! This command groups messages by topic and checks timeline ordering both per chunk and across the
//! entire topic.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Context as _;
use parking_lot::Mutex;

use re_log_types::{TimeType, TimelineName};
use re_mcap::decoders::{DecoderRegistry, TopicFilter};

use super::info::print_table;

#[derive(Debug, Clone, clap::Parser)]
pub struct CheckCommand {
    /// Path to the .mcap file to check.
    path: PathBuf,

    /// Check timelines produced by the full `re_mcap` decoder pipeline.
    ///
    /// This decodes message payloads to include derived timelines, such as `ros2_timestamp` for
    /// ROS 2 data, and increases processing time.
    #[clap(long)]
    full: bool,

    /// Reconstruct a missing or invalid summary from the readable portion of the file.
    #[clap(long)]
    recover: bool,
}

impl CheckCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path,
            full,
            recover,
        } = self;

        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open MCAP file\nFile path: {}", path.display()))?;
        // SAFETY: The map is read-only and remains alive for the duration of the command.
        #[expect(unsafe_code)]
        let mmap = unsafe { memmap2::Mmap::map(&file) }.with_context(|| {
            format!(
                "Failed to memory-map MCAP file\nFile path: {}",
                path.display()
            )
        })?;
        let mcap_file = re_mcap::McapFile::new(mmap, *recover);
        let summary = mcap_file.summary().with_context(|| {
            format!("Failed to inspect MCAP file\nFile path: {}", path.display())
        })?;
        let by_topic = if *full {
            collect_by_topic_full(mcap_file.bytes(), &summary)?
        } else {
            collect_by_topic_raw(mcap_file.bytes(), &summary)?
        };

        println!("{}", path.display());
        print_timeline_checks(&by_topic, &summary, *full);
        Ok(())
    }
}

fn print_timeline_checks(by_topic: &ByTopic, summary: &mcap::Summary, full: bool) {
    let timeline_names = timeline_names(by_topic);
    let channel_id_by_topic: BTreeMap<&str, u16> = summary
        .channels
        .iter()
        .map(|(id, channel)| (channel.topic.as_str(), *id))
        .collect();
    let mut rows = Vec::new();

    for (topic, chunks) in by_topic {
        let num_conflicting_chunks = chunks
            .iter()
            .filter(|times| !times.timelines_agree())
            .count();
        let mut whole_topic = TimeColumns::default();
        for times in chunks {
            whole_topic.append(times);
        }

        let mut issues = Vec::new();
        if num_conflicting_chunks > 0 {
            issues.push(format!(
                "{num_conflicting_chunks} chunk row-order conflicts"
            ));
        }
        if !whole_topic.timelines_agree() {
            issues.push("whole-topic row-order conflict".to_owned());
        }
        for (timeline, count) in unordered_chunk_counts(chunks, &timeline_names) {
            if count > 0 {
                issues.push(format!("{count} unordered chunks on {timeline}"));
            }
        }

        rows.push(vec![
            if issues.is_empty() { "ok" } else { "PROBLEM" }.to_owned(),
            channel_id_by_topic
                .get(topic.as_str())
                .map_or_else(|| "?".to_owned(), u16::to_string),
            chunks.len().to_string(),
            topic.clone(),
            if issues.is_empty() {
                "—".to_owned()
            } else {
                issues.join(", ")
            },
        ]);
    }
    rows.sort_by_key(|row| row[1].parse::<u16>().unwrap_or(u16::MAX));

    println!();
    println!(
        "Timelines: {}",
        timeline_names
            .iter()
            .map(TimelineName::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !full {
        println!("Run with --full to decode all timelines.");
    }
    println!();
    print_table(
        &["Status", "ID", "Chunks", "Topic", "Issues"],
        &rows,
        &[1, 2],
    );
}

/// Times for a set of messages on one topic, keyed by timeline name.
///
/// Row `i` across all column vectors refers to the same message.
#[derive(Default)]
struct TimeColumns {
    columns: BTreeMap<TimelineName, Vec<i64>>,
}

impl TimeColumns {
    fn push_pairs(&mut self, pairs: impl IntoIterator<Item = (TimelineName, i64)>) {
        for (name, v) in pairs {
            self.columns.entry(name).or_default().push(v);
        }
    }

    fn append(&mut self, other: &Self) {
        for (k, vs) in &other.columns {
            self.columns.entry(*k).or_default().extend_from_slice(vs);
        }
    }

    fn len(&self) -> usize {
        self.columns.values().next().map_or(0, Vec::len)
    }

    /// Stable lex sort permutation across all timelines (matching
    /// [`re_chunk::Chunk::from_auto_row_ids`]).
    fn sorted_permutation(&self) -> Vec<usize> {
        let count = self.len();
        let cols: Vec<&Vec<i64>> = self.columns.values().collect();
        let mut perm: Vec<usize> = (0..count).collect();
        perm.sort_by(|&a, &b| {
            for col in &cols {
                let ord = col[a].cmp(&col[b]);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        });
        perm
    }

    /// Do all timelines agree on a single row order?
    ///
    /// Equivalent to: after lex-sorting rows by all timelines, is every individual
    /// timeline non-decreasing? If false, no row permutation can keep all
    /// [`re_chunk::TimeColumn`]s sorted simultaneously: they conflict.
    fn timelines_agree(&self) -> bool {
        if self.len() < 2 {
            return true;
        }
        let perm = self.sorted_permutation();
        self.columns
            .values()
            .all(|col| perm.windows(2).all(|w| col[w[0]] <= col[w[1]]))
    }
}

type ByTopic = BTreeMap<String, Vec<TimeColumns>>;

/// Raw mode: walk MCAP messages directly; only `message_log_time`/`message_publish_time`
/// are available. Grouped by (topic, mcap chunk).
fn collect_by_topic_raw(bytes: &[u8], summary: &mcap::Summary) -> anyhow::Result<ByTopic> {
    let mut by_topic_chunk: BTreeMap<String, BTreeMap<usize, TimeColumns>> = BTreeMap::new();
    for (mcap_idx, chunk) in summary.chunk_indexes.iter().enumerate() {
        for msg in summary.stream_chunk(bytes, chunk)? {
            let msg = msg?;
            by_topic_chunk
                .entry(msg.channel.topic.clone())
                .or_default()
                .entry(mcap_idx)
                .or_default()
                .push_pairs([
                    (
                        TimelineName::from("message_log_time"),
                        msg.log_time.cast_signed(),
                    ),
                    (
                        TimelineName::from("message_publish_time"),
                        msg.publish_time.cast_signed(),
                    ),
                ]);
        }
    }
    Ok(by_topic_chunk
        .into_iter()
        .map(|(topic, chunks)| (topic, chunks.into_values().collect()))
        .collect())
}

/// Full mode: run the decoder pipeline and inspect every rerun chunk it emits.
/// Picks up extra timelines added by decoders (e.g. `ros2_timestamp`).
fn collect_by_topic_full(bytes: &[u8], summary: &mcap::Summary) -> anyhow::Result<ByTopic> {
    let plan =
        DecoderRegistry::all_with_raw_fallback().plan(bytes, summary, &TopicFilter::default())?;

    let chunks: Mutex<Vec<re_chunk::Chunk>> = Mutex::new(Vec::new());
    plan.run(bytes, summary, TimeType::TimestampNs, &|chunk| {
        chunks.lock().push(chunk);
    })?;
    let chunks = chunks.into_inner();

    let mut by_topic: ByTopic = BTreeMap::new();
    for chunk in chunks {
        if chunk.timelines().is_empty() {
            // Static chunk — no timelines to analyze.
            continue;
        }
        let topic = chunk.entity_path().to_string();
        let mut times = TimeColumns::default();
        for (name, time_col) in chunk.timelines() {
            let column = times.columns.entry(*name).or_default();
            column.extend_from_slice(time_col.times_raw());
        }
        by_topic.entry(topic).or_default().push(times);
    }
    Ok(by_topic)
}

/// Per timeline, count chunks (in mcap arrival order) whose min time falls below the
/// running max of all preceding chunks, i.e. chunks that are not in monotone time order
/// on that timeline.
fn unordered_chunk_counts(
    chunks: &[TimeColumns],
    timelines: &[TimelineName],
) -> Vec<(TimelineName, usize)> {
    timelines
        .iter()
        .map(|tl| {
            let mut prev_max: Option<i64> = None;
            let mut unordered = 0usize;
            for tc in chunks {
                let Some(col) = tc.columns.get(tl) else {
                    continue;
                };
                let Some(&min) = col.iter().min() else {
                    continue;
                };
                let max = *col.iter().max().expect("col non-empty");
                if let Some(p) = prev_max
                    && min < p
                {
                    unordered += 1;
                }
                prev_max = Some(prev_max.map_or(max, |p| p.max(max)));
            }
            (*tl, unordered)
        })
        .collect()
}

fn timeline_names(by_topic: &ByTopic) -> Vec<TimelineName> {
    let mut names: std::collections::BTreeSet<TimelineName> = std::collections::BTreeSet::new();
    for chunks in by_topic.values() {
        for tc in chunks {
            names.extend(tc.columns.keys().copied());
        }
    }
    names.into_iter().collect()
}
