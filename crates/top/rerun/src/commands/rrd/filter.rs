use std::{collections::HashSet, io::IsTerminal};

use anyhow::Context as _;

use re_build_info::CrateVersion;
use re_chunk::{external::crossbeam, TransportChunk};

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct FilterCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Names of the timelines to be filtered out.
    #[clap(long = "timeline")]
    dropped_timelines: Vec<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long, default_value_t = false)]
    best_effort: bool,
}

impl FilterCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !std::io::stdout().is_terminal(),
            "you must redirect the output to a file and/or stream"
        );

        let Self {
            path_to_input_rrds,
            dropped_timelines,
            best_effort,
        } = self;

        let now = std::time::Instant::now();
        re_log::info!(srcs = ?path_to_input_rrds, ?dropped_timelines, "filter started");

        let dropped_timelines: HashSet<_> = dropped_timelines.iter().collect();

        // TODO(cmc): might want to make this configurable at some point.
        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let (rx_decoder, rx_size_bytes) =
            read_rrd_streams_from_file_or_stdin(version_policy, path_to_input_rrds);

        // TODO(cmc): might want to make this configurable at some point.
        let (tx_encoder, rx_encoder) = crossbeam::channel::bounded(100);

        let encoding_handle = std::thread::Builder::new()
            .name("rerun-rrd-filter-out".to_owned())
            .spawn(move || -> anyhow::Result<u64> {
                use std::io::Write as _;

                let mut rrd_out = std::io::BufWriter::new(std::io::stdout().lock());

                let mut encoder = {
                    // TODO(cmc): encoding options & version should match the original.
                    let version = CrateVersion::LOCAL;
                    let options = re_log_encoding::EncodingOptions::COMPRESSED;
                    re_log_encoding::encoder::Encoder::new(version, options, &mut rrd_out)
                        .context("couldn't init encoder")?
                };

                let mut size_bytes = 0;
                for msg in rx_encoder {
                    size_bytes += encoder.append(&msg).context("encoding failure")?;
                }

                rrd_out.flush().context("couldn't flush output")?;

                Ok(size_bytes)
            });

        for res in rx_decoder {
            let mut is_success = true;

            match res {
                Ok(msg) => {
                    let msg = match msg {
                        re_log_types::LogMsg::ArrowMsg(store_id, mut msg) => {
                            let (fields, columns): (Vec<_>, Vec<_>) =
                                itertools::izip!(msg.schema.fields.iter(), msg.chunk.iter())
                                    .filter(|(field, _col)| {
                                        filter_timeline(&dropped_timelines, field)
                                    })
                                    .map(|(field, col)| (field.clone(), col.clone()))
                                    .unzip();

                            msg.schema.fields = fields;
                            msg.chunk = re_log_types::external::arrow2::chunk::Chunk::new(columns);

                            re_log_types::LogMsg::ArrowMsg(store_id, msg)
                        }

                        msg => msg,
                    };

                    tx_encoder.send(msg).ok();
                }

                Err(err) => {
                    re_log::error!(err = re_error::format(err));
                    is_success = false;
                }
            }

            if !*best_effort && !is_success {
                anyhow::bail!(
                    "one or more IO and/or decoding failures in the input stream (check logs)"
                )
            }
        }

        std::mem::drop(tx_encoder);
        let rrd_out_size = encoding_handle
            .context("couldn't spawn IO thread")?
            .join()
            .unwrap()?;

        let rrds_in_size = rx_size_bytes.recv().ok();
        let filtered_ratio =
            if let (Some(rrds_in_size), rrd_out_size) = (rrds_in_size, rrd_out_size) {
                format!(
                    "{:3.3}%",
                    100.0 - rrd_out_size as f64 / (rrds_in_size as f64 + f64::EPSILON) * 100.0
                )
            } else {
                "N/A".to_owned()
            };

        let file_size_to_string = |size: Option<u64>| {
            size.map_or_else(
                || "<unknown>".to_owned(),
                |size| re_format::format_bytes(size as _),
            )
        };

        re_log::info!(
            dst_size_bytes = %file_size_to_string(Some(rrd_out_size)),
            time = ?now.elapsed(),
            filtered_ratio,
            srcs = ?path_to_input_rrds,
            srcs_size_bytes = %file_size_to_string(rrds_in_size),
            "filter finished"
        );

        Ok(())
    }
}

// ---

use re_sdk::external::arrow2::datatypes::Field as ArrowField;

fn filter_timeline(dropped_timelines: &HashSet<&String>, field: &ArrowField) -> bool {
    let is_timeline = field
        .metadata
        .get(TransportChunk::FIELD_METADATA_KEY_KIND)
        .map(|s| s.as_str())
        == Some(TransportChunk::FIELD_METADATA_VALUE_KIND_TIME);

    let is_dropped = dropped_timelines.contains(&field.name);

    !is_timeline || !is_dropped
}
