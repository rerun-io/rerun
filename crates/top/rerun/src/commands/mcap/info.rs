//! `rerun mcap info` — inspect timeline structure of an MCAP file.
//!
//! Each MCAP chunk holds messages from many topics interleaved together. When loaded
//! into Rerun, an MCAP chunk is split per topic into one rerun chunk per (topic, mcap chunk),
//! and each rerun chunk gets its timelines reordered via [`re_chunk::Chunk::from_auto_row_ids`]
//! (stable lex sort across all timelines). A rerun chunk's secondary timelines stay sorted
//! only if every timeline agrees on the row order; otherwise the chunk ends up with some
//! `TimeColumn::is_sorted() == false`.
//!
//! This command groups messages by topic and runs that same check both per chunk and
//! across the entire topic.
//!
//! By default only the timelines available at the raw MCAP level
//! (`message_log_time`, `message_publish_time`) are inspected. With `--full`, the
//! `re_mcap` decoder pipeline runs so that timelines extracted from message bodies
//! (e.g. `ros2_timestamp` from a ROS 2 message `Header.stamp`, `timestamp` from custom
//! decoders) are inspected too.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Context as _;
use parking_lot::Mutex;

use re_log_types::{TimeType, TimelineName};
use re_mcap::decoders::{DecoderRegistry, TopicFilter};
use re_mcap::read_summary;

#[derive(Debug, Clone, clap::Parser)]
pub struct InfoCommand {
    /// Path to the .mcap file to inspect.
    path: PathBuf,

    /// Run the full `re_mcap` decoder pipeline.
    ///
    /// Surfaces timelines added by per-message decoders (e.g. `ros2_timestamp` from
    /// a ROS 2 `Header.stamp`). Without this flag only the raw MCAP-level timelines
    /// `message_log_time` / `message_publish_time` are inspected.
    #[clap(long)]
    full: bool,
}

impl InfoCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self { path, full } = self;

        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;

        let summary = read_summary(std::io::Cursor::new(&bytes[..]))?
            .context("MCAP file has no summary section")?;

        let by_topic = if *full {
            collect_by_topic_full(&bytes, &summary)?
        } else {
            collect_by_topic_raw(&bytes, &summary)?
        };

        let timeline_names = timeline_names(&by_topic);

        println!("File:        {}", path.display());
        println!("Channels:    {}", summary.channels.len());
        println!("MCAP chunks: {}", summary.chunk_indexes.len());
        println!(
            "Mode:        {}",
            if *full {
                "full (decoder pipeline)"
            } else {
                "raw"
            }
        );
        println!(
            "Timelines:   {}",
            timeline_names
                .iter()
                .map(TimelineName::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!();
        println!("Per-topic line format:");
        println!("  <status>  id=<N>  topic=<…>  chunks=<N>  [issues…]");
        println!();
        println!("Possible issues:");
        println!("  - N chunks with row-order conflicts: timelines within a chunk disagree on row");
        println!("    order, so no row permutation keeps every TimeColumn sorted simultaneously.");
        println!("  - whole-topic row-order conflict: the same conflict when all messages on the");
        println!("    topic are concatenated together.");
        println!(
            "  - N unordered chunks on <timeline>: chunks (in mcap arrival order) whose min time"
        );
        println!(
            "    falls below the running max on this timeline, i.e. chunks not sorted by time."
        );
        println!(
            "    Independent of row-order conflicts: a chunk can be internally consistent and"
        );
        println!("    still arrive out of order relative to its predecessors.");
        println!();

        let channel_id_by_topic: BTreeMap<&str, u16> = summary
            .channels
            .iter()
            .map(|(id, ch)| (ch.topic.as_str(), *id))
            .collect();

        for (topic, chunks) in &by_topic {
            let num_chunks = chunks.len();
            let num_conflicting_chunks = chunks.iter().filter(|t| !t.timelines_agree()).count();

            let mut whole_topic = TimeColumns::default();
            for tc in chunks {
                whole_topic.append(tc);
            }
            let whole_topic_conflict = !whole_topic.timelines_agree();

            let unordered = unordered_chunk_counts(chunks, &timeline_names);

            let mut issues: Vec<String> = Vec::new();
            if num_conflicting_chunks > 0 {
                issues.push(format!(
                    "{num_conflicting_chunks} chunks with row-order conflicts"
                ));
            }
            if whole_topic_conflict {
                issues.push("whole-topic row-order conflict".to_owned());
            }
            for (tl, n) in &unordered {
                if *n > 0 {
                    issues.push(format!("{n} unordered chunks on {tl}"));
                }
            }

            let status = if issues.is_empty() { "ok" } else { "PROBLEM" };
            let issues_str = issues.join(", ");

            let channel_id = channel_id_by_topic
                .get(topic.as_str())
                .map_or_else(|| "?".to_owned(), u16::to_string);

            println!(
                "{status:<7} id={channel_id:<3} topic={topic:<48} \
                 chunks={num_chunks:<4} {issues_str}"
            );
        }

        Ok(())
    }
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
                        TimelineName::new("message_log_time"),
                        msg.log_time.cast_signed(),
                    ),
                    (
                        TimelineName::new("message_publish_time"),
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
