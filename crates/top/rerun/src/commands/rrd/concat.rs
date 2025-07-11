use std::io::{IsTerminal as _, Write as _};

use anyhow::Context as _;
use itertools::Either;
use re_chunk_store::ChunkStoreConfig;
use re_entity_db::EntityDb;
use re_sdk::ApplicationId;

use crate::commands::read_rrd_streams_from_file_or_stdin;

#[derive(Debug, Clone, clap::Parser)]
pub struct ConcatCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    ///
    /// The id of the resulting recording is specified by the first input file.
    path_to_input_rrds: Vec<String>,

    /// Path to write to. Writes to standard output if unspecified.
    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: Option<String>,

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = false)]
    continue_on_error: bool,

    /// If set, specifies the application id of the resulting recording.
    #[clap(long = "application-id")]
    application_id: Option<String>,
}

impl ConcatCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
            continue_on_error,
            application_id,
        } = self;

        if path_to_output_rrd.is_none() {
            anyhow::ensure!(
                !std::io::stdout().is_terminal(),
                "you must redirect the output to a file and/or stream"
            );
        }

        let store_config = ChunkStoreConfig::ALL_DISABLED;

        let maybe_application_id = application_id.clone().map(Into::into);

        concat(
            *continue_on_error,
            maybe_application_id.as_ref(),
            &store_config,
            path_to_input_rrds,
            path_to_output_rrd.as_ref(),
        )
    }
}

// TODO: Blueprint handling!
fn concat(
    continue_on_error: bool,
    maybe_application_id: Option<&ApplicationId>,
    store_config: &ChunkStoreConfig,
    path_to_input_rrds: &[String],
    path_to_output_rrd: Option<&String>,
) -> anyhow::Result<()> {
    let file_size_to_string = |size: Option<u64>| {
        size.map_or_else(
            || "<unknown>".to_owned(),
            |size| re_format::format_bytes(size as _),
        )
    };

    let now = std::time::Instant::now();
    re_log::info!(
        max_rows = %re_format::format_uint(store_config.chunk_max_rows),
        max_rows_if_unsorted = %re_format::format_uint(store_config.chunk_max_rows_if_unsorted),
        max_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
        srcs = ?path_to_input_rrds,
        "concat started"
    );

    let (rx, rx_size_bytes) = read_rrd_streams_from_file_or_stdin(path_to_input_rrds);

    let mut recording_entity_db: Option<EntityDb> = None;
    let mut blueprint_entity_db: Option<EntityDb> = None;

    for (_source, res) in rx {
        let mut is_success = true;

        match res {
            Ok(mut msg) => {
                let entity_db = match msg.store_id().kind {
                    re_sdk::StoreKind::Recording => recording_entity_db.get_or_insert_with(|| {
                        let store_id = blueprint_entity_db.as_ref().map_or(
                            msg.store_id().clone(),
                            |blueprint_id| re_sdk::StoreId {
                                kind: re_sdk::StoreKind::Recording,
                                id: blueprint_id.store_id().id,
                            },
                        );

                        re_entity_db::EntityDb::with_store_config(store_id, store_config.clone())
                    }),
                    re_sdk::StoreKind::Blueprint => blueprint_entity_db.get_or_insert_with(|| {
                        let store_id = recording_entity_db.as_ref().map_or(
                            msg.store_id().clone(),
                            |recording_id| re_sdk::StoreId {
                                kind: re_sdk::StoreKind::Blueprint,
                                id: recording_id.store_id().id,
                            },
                        );
                        re_entity_db::EntityDb::with_store_config(store_id, store_config.clone())
                    }),
                };

                // We need to rewire the ids the messages too.
                let id = entity_db.store_id().id;
                match &mut msg {
                    re_log_types::LogMsg::SetStoreInfo(set_store_info) => {
                        set_store_info.info.store_id.id = id;
                        if let Some(application_id) = maybe_application_id {
                            set_store_info.info.application_id = application_id.clone();
                        }
                    }
                    re_log_types::LogMsg::ArrowMsg(store_id, _) => {
                        store_id.id = id;
                    }
                    re_log_types::LogMsg::BlueprintActivationCommand(
                        blueprint_activation_command,
                    ) => blueprint_activation_command.blueprint_id.id = id,
                };

                if let Err(err) = entity_db.add(&msg) {
                    re_log::error!(%err, "couldn't index corrupt chunk");
                    is_success = false;
                };
            }
            Err(err) => {
                re_log::error!(err = re_error::format(err));
                is_success = false;
            }
        }
        if !continue_on_error && !is_success {
            anyhow::bail!(
                "one or more IO and/or decoding failures in the input stream (check logs)"
            )
        }
    }

    let mut rrd_out = if let Some(path) = path_to_output_rrd {
        Either::Left(std::io::BufWriter::new(
            std::fs::File::create(path).with_context(|| format!("{path:?}"))?,
        ))
    } else {
        Either::Right(std::io::BufWriter::new(std::io::stdout().lock()))
    };

    let messages_rbl = blueprint_entity_db
        .iter()
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let messages_rrd = recording_entity_db
        .iter()
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    // TODO(grtlr): encoding options should match the original.
    let encoding_options = re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = blueprint_entity_db
        .as_ref()
        .or(recording_entity_db.as_ref())
        .and_then(|db| db.store_info())
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);
    let rrd_out_size = re_log_encoding::encoder::encode(
        version,
        encoding_options,
        // NOTE: We want to make sure all blueprints come first, so that the viewer can immediately
        // set up the viewport correctly.
        messages_rbl.chain(messages_rrd),
        &mut rrd_out,
    )
    .context("couldn't encode messages")?;

    rrd_out.flush().context("couldn't flush output")?;

    let rrds_in_size = rx_size_bytes.recv().ok();
    let size_reduction = if let (Some(rrds_in_size), rrd_out_size) = (rrds_in_size, rrd_out_size) {
        format!(
            "-{:3.3}%",
            100.0 - rrd_out_size as f64 / (rrds_in_size as f64 + f64::EPSILON) * 100.0
        )
    } else {
        "N/A".to_owned()
    };

    re_log::info!(
        dst_size_bytes = %file_size_to_string(Some(rrd_out_size)),
        time = ?now.elapsed(),
        size_reduction,
        srcs = ?path_to_input_rrds,
        srcs_size_bytes = %file_size_to_string(rrds_in_size),
        "concat finished"
    );

    Ok(())
}
