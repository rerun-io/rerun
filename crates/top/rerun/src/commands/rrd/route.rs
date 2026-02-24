use std::fs::File;
use std::io::BufWriter;

use crossbeam::channel::Receiver;
use itertools::Itertools as _;

use re_chunk::ChunkId;
use re_log_encoding::{Encoder, RawRrdManifest};
use re_protos::common::v1alpha1::ApplicationId;
use re_protos::log_msg::v1alpha1::log_msg::Msg;
use re_protos::log_msg::v1alpha1::{ArrowMsg, BlueprintActivationCommand, SetStoreInfo, StoreInfo};

use crate::commands::read_raw_rrd_streams_from_file_or_stdin;
use crate::commands::stdio::InputSource;

#[derive(Debug, Clone, clap::Parser)]
pub struct RouteCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.rrd")]
    path_to_output_rrd: Option<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,

    /// If set, specifies the application id of the output.
    #[clap(long = "application-id")]
    application_id: Option<String>,

    /// If set, specifies the recording id of the output.
    ///
    /// When this flag is set and multiple input .rdd files are specified,
    /// blueprint activation commands will be dropped from the resulting
    /// output.
    #[clap(long = "recording-id")]
    recording_id: Option<String>,
}

struct Rewrites {
    application_id: Option<ApplicationId>,
    recording_id: Option<String>,
}

impl RouteCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            continue_on_error,
            application_id,
            recording_id,
        } = self;

        let rewrites = Rewrites {
            application_id: application_id
                .as_ref()
                .map(|id| ApplicationId { id: id.clone() }),
            recording_id: recording_id.clone(),
        };

        let (rx, rx_footers) = read_raw_rrd_streams_from_file_or_stdin(path_to_input_rrds);

        // When we merge multiple recordings with blueprints, it does not make sense to activate any of them,
        // and instead we want viewer heuristics to take over. Therefore, we drop blueprint activation
        // commands when overwriting the recording id.
        let drop_blueprint_activation_cmds =
            path_to_input_rrds.len() > 1 && rewrites.recording_id.is_some();

        if let Some(path) = path_to_output_rrd {
            let writer = BufWriter::new(File::create(path)?);
            process_messages(
                &rewrites,
                *continue_on_error,
                writer,
                &rx,
                &rx_footers,
                drop_blueprint_activation_cmds,
            )?;
        } else {
            let stdout = std::io::stdout();
            let lock = stdout.lock();
            let writer = BufWriter::new(lock);
            process_messages(
                &rewrites,
                *continue_on_error,
                writer,
                &rx,
                &rx_footers,
                drop_blueprint_activation_cmds,
            )?;
        }

        Ok(())
    }
}

#[expect(clippy::fn_params_excessive_bools)] // private function 🤷‍♂️
#[expect(clippy::type_complexity)] // private function 🤷‍♂️
fn process_messages<W: std::io::Write>(
    rewrites: &Rewrites,
    continue_on_error: bool,
    writer: W,
    rx: &Receiver<(InputSource, anyhow::Result<Msg>)>,
    rx_footers: &Receiver<(u64, Vec<(InputSource, anyhow::Result<RawRrdManifest>)>)>,
    drop_blueprint_activation_cmds: bool,
) -> anyhow::Result<()> {
    re_log::info!("processing input…");
    let mut num_total_msgs = 0;
    let mut num_unexpected_msgs = 0;
    let mut num_blueprint_activations = 0;

    // TODO(grtlr): encoding should match the original (just like in `rrd stats`).
    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = re_build_info::CrateVersion::LOCAL;
    let mut encoder = Encoder::new_eager(version, options, writer)?;

    // TODO(cmc): this can be optimized to not keep all the values in memory in the multi-store case.
    let mut chunk_ids = Vec::new();
    let mut byte_offsets_excluding_header = Vec::new();
    let mut byte_sizes_excluding_header = Vec::new();
    let mut byte_sizes_uncompressed = Vec::new();

    while let Ok((_input, res)) = rx.recv() {
        let mut is_success = true;

        match res {
            Ok(mut msg) => {
                num_total_msgs += 1;

                #[expect(deprecated)]
                match &mut msg {
                    // This needs to come first, as an
                    Msg::BlueprintActivationCommand(_) if drop_blueprint_activation_cmds => {
                        num_blueprint_activations += 1;
                        continue;
                    }

                    Msg::SetStoreInfo(SetStoreInfo {
                        info:
                            Some(StoreInfo {
                                store_id,
                                application_id: _, // deprecated but not considered.
                                store_source: _,
                                store_version: _,
                            }),
                        row_id: _,
                    })
                    | Msg::BlueprintActivationCommand(BlueprintActivationCommand {
                        blueprint_id: store_id,
                        make_active: _,
                        make_default: _,
                    })
                    | Msg::ArrowMsg(ArrowMsg {
                        store_id,
                        chunk_id: _,
                        compression: _,
                        uncompressed_size: _,
                        encoding: _,
                        payload: _,
                        is_static: _,
                    }) => {
                        if let Some(target_store_id) = store_id {
                            if let Some(recording_id) = &rewrites.recording_id {
                                target_store_id.recording_id = recording_id.clone();
                            }

                            if let Some(application_id) = &rewrites.application_id {
                                target_store_id.application_id = Some(application_id.clone());
                            }
                        }
                    }

                    Msg::SetStoreInfo(SetStoreInfo {
                        row_id: _,
                        info: None,
                    }) => {
                        num_unexpected_msgs += 1;
                        is_success = false;
                        re_log::warn_once!(
                            "Encountered `SetStoreInfo` without `info` field: {:#?}",
                            msg
                        );
                    }
                }

                // Safety: we're just forwarding an existing message, we didn't change its payload
                // in any meaningful way.
                #[expect(unsafe_code)]
                let (byte_span_excluding_header, byte_size_uncompressed) = unsafe {
                    // Reminder: this will implicitly discard RRD footers.
                    encoder.append_transport(&msg)?
                };

                if let re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(arrow_msg) = msg {
                    chunk_ids.push(arrow_msg.chunk_id.expect("chunk must have a chunk ID"));
                    byte_offsets_excluding_header.push(byte_span_excluding_header.start);
                    byte_sizes_excluding_header.push(byte_span_excluding_header.len);
                    byte_sizes_uncompressed.push(byte_size_uncompressed);
                }
            }
            Err(err) => {
                re_log::error_once!("{}", re_error::format(err));
                is_success = false;
            }
        }

        if !continue_on_error && !is_success {
            anyhow::bail!(
                "one or more IO and/or decoding failures in the input stream (check logs)"
            )
        }
    }

    let mut rrd_footer = re_log_encoding::RrdFooter::default();
    let mut i = 0;
    for (_, manifests) in rx_footers {
        for (_, res) in manifests {
            let manifest = res?;

            let RawRrdManifest {
                store_id,
                sorbet_schema,
                sorbet_schema_sha256,
                data,
            } = manifest.clone();

            let patched_store_id = re_log_types::StoreId::new(
                store_id.kind(),
                rewrites
                    .application_id
                    .clone()
                    .unwrap_or_else(|| store_id.application_id().clone().into()),
                rewrites
                    .recording_id
                    .clone()
                    .unwrap_or_else(|| store_id.recording_id().to_string()),
            );

            let byte_offsets = &byte_offsets_excluding_header[i..i + data.num_rows()];
            let byte_sizes = &byte_sizes_excluding_header[i..i + data.num_rows()];
            let byte_sizes_uncompressed = &byte_sizes_uncompressed[i..i + data.num_rows()];

            // NOTE: All of this works because our CLI tools guarantee that while the data and the
            // footers will be received at a different time, they'll still follow the same global order.
            // Still, we double check the chunk IDs in order to make sure that they still align.
            let chunk_ids = &chunk_ids[i..i + data.num_rows()];
            for (chunk_id, expected_chunk_id) in
                itertools::izip!(chunk_ids, manifest.col_chunk_id()?)
            {
                assert_eq!(
                    *chunk_id,
                    (*expected_chunk_id).into(),
                    "[i={i}] {expected_chunk_id} != {}: {:#?}",
                    ChunkId::from_tuid((*chunk_id).try_into().expect("must be valid TUID")),
                    manifest.col_chunk_id()?.take(5).collect_vec(),
                );
            }

            i += data.num_rows();

            use arrow::array::{ArrayRef, UInt64Array};
            use std::sync::Arc;
            let column_byte_offsets =
                Arc::new(UInt64Array::from(byte_offsets.to_vec())) as ArrayRef;
            let column_byte_sizes = Arc::new(UInt64Array::from(byte_sizes.to_vec())) as ArrayRef;
            let column_byte_sizes_uncompressed =
                Arc::new(UInt64Array::from(byte_sizes_uncompressed.to_vec())) as ArrayRef;

            let (schema, mut columns, num_rows) = data.into_parts();
            for (field, column) in itertools::izip!(schema.fields(), &mut columns) {
                match field.name().as_str() {
                    RawRrdManifest::FIELD_CHUNK_BYTE_OFFSET => {
                        *column = column_byte_offsets.clone();
                    }

                    RawRrdManifest::FIELD_CHUNK_BYTE_SIZE => {
                        *column = column_byte_sizes.clone();
                    }

                    RawRrdManifest::FIELD_CHUNK_BYTE_SIZE_UNCOMPRESSED => {
                        *column = column_byte_sizes_uncompressed.clone();
                    }

                    _ => {}
                }
            }

            let data = arrow::array::RecordBatch::try_new_with_options(
                schema,
                columns,
                &arrow::array::RecordBatchOptions::new().with_row_count(Some(num_rows)),
            )?;

            // NOTE: We currently enforce a single RRD footer per recording to keep things
            // manageable end-to-end, therefore we must merge the manifests if multiple recordings
            // get routed to the same ID.
            match rrd_footer.manifests.entry(patched_store_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let existing_manifest = e.get_mut();
                    assert_eq!(existing_manifest.store_id, patched_store_id);

                    existing_manifest.sorbet_schema = arrow::datatypes::Schema::try_merge([
                        existing_manifest.sorbet_schema.clone(),
                        sorbet_schema.clone(),
                    ])?;

                    existing_manifest.data = arrow::compute::concat_batches(
                        &existing_manifest.data.schema(),
                        &[existing_manifest.data.clone(), data.clone()],
                    )?;
                }

                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(RawRrdManifest {
                        store_id: patched_store_id,
                        sorbet_schema,
                        sorbet_schema_sha256,
                        data,
                    });
                }
            }
        }
    }

    // Only perform this once, after all concatenations are done.
    for manifest in rrd_footer.manifests.values_mut() {
        manifest.sorbet_schema_sha256 =
            RawRrdManifest::compute_sorbet_schema_sha256(&manifest.sorbet_schema)?;
    }

    // Safety: this entire function is about making this call safe.
    #[expect(unsafe_code)]
    unsafe {
        encoder.finish_with_custom_footer(&rrd_footer)?;
    }

    re_log::info_once!(
        "Processed {num_total_msgs} messages, dropped {num_blueprint_activations} blueprint activations, and encountered {num_unexpected_msgs} unexpected messages."
    );
    Ok(())
}
