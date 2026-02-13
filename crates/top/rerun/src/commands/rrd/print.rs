use anyhow::Context as _;
use arrow::array::RecordBatch;
use itertools::Itertools as _;
use re_arrow_util::RecordBatchExt as _;
use re_byte_size::SizeBytes as _;
use re_log_types::{LogMsg, SetStoreInfo};
use re_sdk::EntityPath;

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct PrintCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// If set, print out table contents.
    ///
    /// This can be specified more than once to toggle more and more verbose levels (e.g. -vvv):
    ///
    /// * default: summary with short names.
    ///
    /// * `-v`: summary with fully-qualified names.
    ///
    /// * `-vv`: show all chunk metadata headers, keep the data hidden.
    ///
    /// * `-vvv`: show all chunk metadata headers as well as the data itself.
    #[clap(long, short, action = clap::ArgAction::Count)]
    verbose: u8,

    // NOTE: we use a hack to allow specifying `=false` or `=true` in CLI. See https://github.com/clap-rs/clap/issues/1649#issuecomment-2144932113
    //
    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long, default_missing_value="true", num_args=0..=1)]
    continue_on_error: Option<bool>,

    /// Migrate chunks to latest version before printing?
    #[clap(long, default_missing_value="true", num_args=0..=1)]
    migrate: Option<bool>,

    /// If true, includes `rerun.` prefixes on keys.
    #[clap(long, default_missing_value="true", num_args=0..=1)]
    full_metadata: Option<bool>,

    /// Show only chunks belonging to this entity.
    #[clap(long)]
    entity: Option<String>,

    /// If true, displays all the parsed footers at the end.
    #[clap(long, default_missing_value="true", num_args=0..=1)]
    footers: Option<bool>,

    /// Transpose record batches before printing them?
    #[clap(long, default_missing_value="true", num_args=0..=1)]
    transposed: Option<bool>,
}

impl PrintCommand {
    pub fn run(self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            continue_on_error,
            verbose,
            migrate,
            full_metadata,
            entity,
            footers,
            transposed,
        } = self;
        let continue_on_error = continue_on_error.unwrap_or(true);

        let migrate = migrate.unwrap_or(true);
        let full_metadata = full_metadata.unwrap_or(false);
        let entity = entity.map(|e| EntityPath::parse_forgiving(&e));
        let footers = footers.unwrap_or(false);
        let transposed = transposed.unwrap_or(false);

        let options = Options {
            verbose,
            migrate,
            transposed,
            full_metadata,
            entity,
        };

        if migrate {
            println!("Showing data after migration to latest Rerun version");
        } else {
            // TODO(#10343): implement this. Requires changing `ArrowMsg` to contain the unmigrated record batch
            panic!(
                "Not implemented - see https://github.com/rerun-io/rerun/issues/10343#issuecomment-3182422629"
            );
        }

        let (rx, rx_done) = read_rrd_streams_from_file_or_stdin(&path_to_input_rrds);

        for (_source, res) in rx {
            let mut is_success = true;

            match res {
                Ok(msg) => {
                    if let Err(err) = print_msg(&options, msg) {
                        re_log::error_once!("{}", re_error::format(err));
                        is_success = false;
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

        if footers {
            for (_, rrd_manifests) in rx_done {
                for (source, mut rrd_manifest) in rrd_manifests? {
                    // Just to be nice: this will display the origin of the data in the header.
                    rrd_manifest
                        .data
                        .schema_metadata_mut()
                        .insert("rerun:source".to_owned(), source.to_string());

                    // Just to be nice: this will display the sorbet schema hash in the header.
                    rrd_manifest.data.schema_metadata_mut().insert(
                        "schema_sha_256".to_owned(),
                        rrd_manifest
                            .sorbet_schema_sha256
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>(),
                    );

                    // Drop all per-entity and/or per-component columns to keep things readable.
                    //
                    // TODO(cmc): more config flags for columns to show etc.
                    let filtered = rrd_manifest
                        .data
                        .filter_columns_by(|f| f.name().starts_with("chunk_"))?;

                    let formatted = re_arrow_util::format_record_batch_opts(
                        &filtered,
                        &re_arrow_util::RecordBatchFormatOpts {
                            max_cell_content_width: 32,
                            ..Default::default()
                        },
                    );

                    println!("{formatted}");
                }
            }
        }

        Ok(())
    }
}

struct Options {
    verbose: u8,
    migrate: bool,
    transposed: bool,
    full_metadata: bool,
    entity: Option<EntityPath>,
}

impl Options {
    fn format_record_batch(&self, full_batch: &RecordBatch) -> impl std::fmt::Display {
        let format_options = re_arrow_util::RecordBatchFormatOpts {
            transposed: self.transposed,
            max_cell_content_width: 100,
            trim_field_names: !self.full_metadata,
            trim_metadata_keys: !self.full_metadata,
            trim_metadata_values: !self.full_metadata,
            ..Default::default()
        };

        if self.verbose <= 2 {
            let empty_batch = full_batch.slice(0, 0);
            re_arrow_util::format_record_batch_opts(&empty_batch, &format_options)
        } else {
            re_arrow_util::format_record_batch_opts(full_batch, &format_options)
        }
    }
}

fn print_msg(options: &Options, msg: LogMsg) -> anyhow::Result<()> {
    match msg {
        LogMsg::SetStoreInfo(msg) => {
            let SetStoreInfo { row_id: _, info } = msg;
            println!("{info:#?}");
        }

        LogMsg::ArrowMsg(_store_id, arrow_msg) => {
            let original_batch = &arrow_msg.batch;

            if options.migrate {
                let migrared_chunk =
                    re_sorbet::ChunkBatch::try_from(original_batch).context("corrupt chunk")?;

                if let Some(only_this_entity) = &options.entity
                    && migrared_chunk.entity_path() != only_this_entity
                {
                    return Ok(()); // not interested in this entity
                }

                print!(
                    "Chunk({}) with {} rows ({}) - {:?} - ",
                    migrared_chunk.chunk_id(),
                    migrared_chunk.num_rows(),
                    re_format::format_bytes(migrared_chunk.total_size_bytes() as _),
                    migrared_chunk.entity_path(),
                );

                match options.verbose {
                    0 => {
                        let column_names = migrared_chunk
                            .component_columns()
                            .map(|(descr, _)| descr.column_name(re_sorbet::BatchType::Chunk)) // short column name without entity-path prefix
                            .join(" ");
                        println!("data columns: [{column_names}]");
                    }
                    1 => {
                        let column_descriptors = migrared_chunk
                            .component_columns()
                            .map(|(descr, _)| descr.to_string())
                            .collect_vec()
                            .join(" ");
                        println!("data columns: [{column_descriptors}]",);
                    }
                    _ => {
                        println!("\n{}\n", options.format_record_batch(&migrared_chunk));
                    }
                }
            } else {
                if let Some(only_this_entity) = &options.entity
                    && let metadata = original_batch.schema_ref().metadata()
                    && let Some(chunk_entity_path) = metadata
                        .get(re_sorbet::metadata::SORBET_ENTITY_PATH)
                        .or_else(|| metadata.get("rerun.entity_path"))
                    && only_this_entity != &EntityPath::parse_forgiving(chunk_entity_path)
                {
                    return Ok(()); // not interested in this entity
                }

                print!(
                    "Chunk with {} rows ({})",
                    original_batch.num_rows(),
                    re_format::format_bytes(original_batch.total_size_bytes() as _),
                );

                match options.verbose {
                    0 | 1 => {
                        let column_names = original_batch
                            .schema()
                            .fields()
                            .iter()
                            .map(|f| f.name())
                            .join(" ");
                        println!("columns: [{column_names}]");
                    }
                    _ => {
                        println!("\n{}\n", options.format_record_batch(original_batch));
                    }
                }
            }
        }

        LogMsg::BlueprintActivationCommand(re_log_types::BlueprintActivationCommand {
            blueprint_id,
            make_active,
            make_default,
        }) => {
            println!(
                "BlueprintActivationCommand({blueprint_id:?}, make_active: {make_active}, make_default: {make_default})"
            );
        }
    }

    Ok(())
}
