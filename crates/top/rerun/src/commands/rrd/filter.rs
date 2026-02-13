use std::collections::HashSet;
use std::io::IsTerminal as _;

use anyhow::Context as _;
use arrow::array::{RecordBatch as ArrowRecordBatch, RecordBatchOptions};
use arrow::datatypes::{Field as ArrowField, Schema as ArrowSchema};
use itertools::Either;
use re_build_info::CrateVersion;
use re_chunk::external::crossbeam;
use re_sdk::EntityPath;
use re_sdk::external::arrow;

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct FilterCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: Option<String>,

    /// Names of the timelines to be filtered out.
    #[clap(long = "drop-timeline")]
    dropped_timelines: Vec<String>,

    /// Paths of the entities to be filtered out.
    #[clap(long = "drop-entity")]
    dropped_entity_paths: Vec<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,
}

impl FilterCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            dropped_timelines,
            dropped_entity_paths,
            continue_on_error,
        } = self;

        let path_to_output_rrd = path_to_output_rrd.clone();
        if path_to_output_rrd.is_none() {
            anyhow::ensure!(
                !std::io::stdout().is_terminal(),
                "you must redirect the output to a file and/or stream"
            );
        }

        let now = std::time::Instant::now();
        re_log::info!(srcs = ?path_to_input_rrds, ?dropped_timelines, "filter started");

        let dropped_timelines: HashSet<_> = dropped_timelines.iter().cloned().collect();
        let dropped_entity_paths: HashSet<EntityPath> = dropped_entity_paths
            .iter()
            .map(|s| EntityPath::parse_forgiving(s))
            .collect();

        let (rx_decoder, rx_size_bytes) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        // TODO(cmc): might want to make this configurable at some point.
        let (tx_encoder, rx_encoder) = crossbeam::channel::bounded(100);

        let encoding_handle = std::thread::Builder::new()
            .name("rerun-rrd-filter-out".to_owned())
            .spawn(move || -> anyhow::Result<u64> {
                use std::io::Write as _;

                let mut rrd_out = if let Some(path) = path_to_output_rrd.as_ref() {
                    Either::Left(std::io::BufWriter::new(
                        std::fs::File::create(path).with_context(|| format!("{path:?}"))?,
                    ))
                } else {
                    Either::Right(std::io::BufWriter::new(std::io::stdout().lock()))
                };

                let mut encoder = {
                    // TODO(cmc): encoding options & version should match the original.
                    let version = CrateVersion::LOCAL;
                    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
                    re_log_encoding::Encoder::new_eager(version, options, &mut rrd_out)
                        .context("couldn't init encoder")?
                };

                let mut size_bytes = 0;
                for msg in rx_encoder {
                    size_bytes += encoder.append(&msg).context("encoding failure")?;
                }

                drop(encoder);
                rrd_out.flush().context("couldn't flush output")?;

                Ok(size_bytes)
            });

        for (_source, res) in rx_decoder {
            let mut is_success = true;

            match res {
                Ok(msg) => {
                    let msg = match msg {
                        re_log_types::LogMsg::ArrowMsg(store_id, mut msg) => {
                            match re_sorbet::ChunkBatch::try_from(&msg.batch) {
                                Ok(batch) => {
                                    if dropped_entity_paths.contains(batch.entity_path()) {
                                        None
                                    } else {
                                        let (fields, columns): (Vec<_>, Vec<_>) = itertools::izip!(
                                            &batch.schema().fields,
                                            batch.columns()
                                        )
                                        .filter(|(field, _col)| {
                                            !is_field_timeline_of(field, &dropped_timelines)
                                        })
                                        .map(|(field, col)| (field.clone(), col.clone()))
                                        .unzip();

                                        if let Ok(new_batch) =
                                            ArrowRecordBatch::try_new_with_options(
                                                ArrowSchema::new_with_metadata(
                                                    fields,
                                                    batch.schema().metadata().clone(),
                                                )
                                                .into(),
                                                columns,
                                                &RecordBatchOptions::default(),
                                            )
                                        {
                                            msg.batch = new_batch;
                                            Some(re_log_types::LogMsg::ArrowMsg(store_id, msg))
                                        } else {
                                            None // Probably failed because we filtered out everything
                                        }
                                    }
                                }
                                Err(err) => {
                                    re_log::warn_once!("Failed to parse chunk schema: {err}");
                                    None
                                }
                            }
                        }

                        msg => Some(msg),
                    };

                    if let Some(msg) = msg {
                        tx_encoder.send(msg).ok();
                    }
                }

                Err(err) => {
                    re_log::error!(err = re_error::format(err));
                    is_success = false;
                }
            }

            if !*continue_on_error && !is_success {
                anyhow::bail!(
                    "one or more IO and/or decoding failures in the input stream (check logs)"
                )
            }
        }

        std::mem::drop(tx_encoder);
        let rrd_out_size = encoding_handle
            .context("couldn't spawn IO thread")?
            .join()
            .map_err(|err| anyhow::anyhow!("Unknown error: {err:?}"))??; // NOLINT: there is no `Display` for this `err`

        let rrds_in_size = rx_size_bytes.recv().ok().map(|(size, _footers)| size);
        let size_reduction =
            if let (Some(rrds_in_size), rrd_out_size) = (rrds_in_size, rrd_out_size) {
                format!(
                    "-{:3.3}%",
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
            size_reduction,
            srcs = ?path_to_input_rrds,
            srcs_size_bytes = %file_size_to_string(rrds_in_size),
            "filter finished"
        );

        Ok(())
    }
}

// ---

// Does the given field represent a timeline that is in the given set?
fn is_field_timeline_of(field: &ArrowField, dropped_timelines: &HashSet<String>) -> bool {
    re_sorbet::IndexColumnDescriptor::try_from(field)
        .ok()
        .is_some_and(|schema| dropped_timelines.contains(schema.column_name()))
}
