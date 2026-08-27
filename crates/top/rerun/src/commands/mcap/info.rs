//! `rerun mcap info` — inspect the structure of an MCAP file.
//!
//! The report is derived from the MCAP header and summary without decompressing message chunks.

use std::path::PathBuf;

use crate::commands::table_util::print_table;
use anyhow::Context as _;

#[derive(Debug, Clone, clap::Parser)]
pub struct InfoCommand {
    /// Path to the .mcap file to inspect.
    path: PathBuf,

    /// Reconstruct a missing or invalid summary from the readable portion of the file.
    #[clap(long)]
    recover: bool,
}

impl InfoCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self { path, recover } = self;

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
        let info = mcap_file.info()?;

        print_info(path, &info);
        Ok(())
    }
}

fn print_info(path: &std::path::Path, info: &re_mcap::McapInfo) {
    println!("{}", path.display());
    println!();
    println!("Recording");
    print_property("Profile", value_or_dash(&info.profile));
    print_property("Library", value_or_dash(&info.library));
    print_property("Messages", format_optional_uint(info.message_count));
    print_property("Duration", format_duration(info.duration_ns));
    print_property("Start", format_timestamp(info.message_start_time_ns));
    print_property("End", format_timestamp(info.message_end_time_ns));
    print_property("Schemas", info.schema_count);
    print_property("Channels", info.channel_count);
    print_property("Attachments", info.attachment_count);
    print_property("Metadata", info.metadata_count);
    print_property(
        "Summary",
        match info.summary_source {
            re_mcap::McapSummarySource::Embedded => "embedded".to_owned(),
            re_mcap::McapSummarySource::Reconstructed => "reconstructed".to_owned(),
        },
    );

    println!();
    println!("Chunks");
    print_property("Count", info.chunks.count);
    print_property(
        "Max uncompressed size",
        format_optional_bytes(info.chunks.max_uncompressed_size_bytes),
    );
    print_property(
        "Max compressed size",
        format_optional_bytes(info.chunks.max_compressed_size_bytes),
    );
    print_property(
        "Overlapping time ranges",
        if info.chunks.has_overlapping_time_ranges {
            "yes"
        } else {
            "no"
        }
        .to_owned(),
    );

    if !info.compression.is_empty() {
        println!();
        println!("Compression");
        let rows = info
            .compression
            .iter()
            .map(|compression| {
                vec![
                    if compression.codec.is_empty() {
                        "none".to_owned()
                    } else {
                        compression.codec.clone()
                    },
                    compression.chunk_count.to_string(),
                    re_format::format_bytes(compression.compressed_size_bytes as f64),
                    re_format::format_bytes(compression.uncompressed_size_bytes as f64),
                    compression
                        .savings_ratio()
                        .map_or_else(|| "—".to_owned(), |ratio| format!("{:.1}%", ratio * 100.0)),
                    info.duration_ns
                        .filter(|duration| *duration > 0)
                        .map_or_else(
                            || "—".to_owned(),
                            |duration| {
                                let bytes_per_second = compression.compressed_size_bytes as f64
                                    / (duration as f64 / 1_000_000_000.0);
                                format!("{}/s", re_format::format_bytes(bytes_per_second))
                            },
                        ),
                ]
            })
            .collect::<Vec<_>>();
        print_table(
            &[
                "Codec",
                "Chunks",
                "Compressed",
                "Uncompressed",
                "Savings",
                "Rate",
            ],
            &rows,
            &[1, 2, 3, 4, 5],
        );
    }

    println!();
    println!("Channels");
    let rows = info
        .channels
        .iter()
        .map(|channel| {
            let schema = channel.schema.as_ref();
            let encoding = match schema {
                Some(schema) if schema.encoding == channel.message_encoding => {
                    schema.encoding.clone()
                }
                Some(schema) => format!("{} / {}", schema.encoding, channel.message_encoding),
                None => channel.message_encoding.clone(),
            };
            vec![
                channel.id.to_string(),
                channel.topic.clone(),
                format_optional_uint(channel.message_count),
                channel.frequency_hz.map_or_else(
                    || "—".to_owned(),
                    |(min, max)| format!("{min:.2}–{max:.2} Hz"),
                ),
                schema.map_or_else(|| "—".to_owned(), |schema| schema.name.clone()),
                value_or_dash(&encoding),
            ]
        })
        .collect::<Vec<_>>();
    print_table(
        &["ID", "Topic", "Messages", "Rate", "Schema", "Encoding"],
        &rows,
        &[0, 2, 3],
    );
}

fn print_property(label: &str, value: impl std::fmt::Display) {
    println!("  {label:<24} {value}");
}

fn value_or_dash(value: &str) -> String {
    if value.is_empty() {
        "—".to_owned()
    } else {
        value.to_owned()
    }
}

fn format_optional_uint(value: Option<u64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| value.to_string())
}

fn format_optional_bytes(value: Option<u64>) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |bytes| re_format::format_bytes(bytes as f64),
    )
}

fn format_duration(duration_ns: Option<u64>) -> String {
    duration_ns
        .and_then(|duration| i64::try_from(duration).ok())
        .map_or_else(
            || "—".to_owned(),
            |duration| {
                re_format::DurationFormatOptions::default()
                    .with_max_decimals(3)
                    .format_nanos(duration)
            },
        )
}

fn format_timestamp(timestamp_ns: Option<u64>) -> String {
    timestamp_ns
        .and_then(|timestamp| i64::try_from(timestamp).ok())
        .map_or_else(
            || "—".to_owned(),
            |timestamp| re_log_types::Timestamp::from_nanos_since_epoch(timestamp).format_iso(),
        )
}
