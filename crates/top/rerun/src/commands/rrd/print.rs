use anyhow::Context as _;
use itertools::Itertools as _;

use re_byte_size::SizeBytes as _;
use re_log_types::{LogMsg, SetStoreInfo};

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

    /// If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
    #[clap(long = "continue-on-error", default_value_t = true)]
    continue_on_error: bool,
}

impl PrintCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            verbose,
            continue_on_error,
        } = self;

        // TODO(cmc): might want to make this configurable at some point.
        let version_policy = re_log_encoding::VersionPolicy::Warn;
        let (rx, _) = read_rrd_streams_from_file_or_stdin(version_policy, path_to_input_rrds);

        for (_source, res) in rx {
            let mut is_success = true;

            match res {
                Ok(msg) => {
                    if let Err(err) = print_msg(*verbose, msg) {
                        re_log::error!(err = re_error::format(err));
                        is_success = false;
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

        Ok(())
    }
}

fn print_msg(verbose: u8, msg: LogMsg) -> anyhow::Result<()> {
    match msg {
        LogMsg::SetStoreInfo(msg) => {
            let SetStoreInfo { row_id: _, info } = msg;
            println!("{info:#?}");
        }

        LogMsg::ArrowMsg(_row_id, arrow_msg) => {
            let mut chunk =
                re_sorbet::ChunkBatch::try_from(&arrow_msg.batch).context("corrupt chunk")?;

            print!(
                "Chunk({}) with {} rows ({}) - {:?} - ",
                chunk.chunk_id(),
                chunk.num_rows(),
                re_format::format_bytes(chunk.total_size_bytes() as _),
                chunk.entity_path(),
            );

            if verbose == 0 {
                let column_names = chunk
                    .component_columns()
                    .map(|(descr, _)| descr.component_name.short_name())
                    .join(" ");
                println!("columns: [{column_names}]");
            } else if verbose == 1 {
                let column_descriptors = chunk
                    .component_columns()
                    .map(|(descr, _)| descr.to_string())
                    .collect_vec()
                    .join(" ");
                println!("columns: [{column_descriptors}]",);
            } else if verbose == 2 {
                chunk = chunk.drop_all_rows();

                let options = re_format_arrow::RecordBatchFormatOpts {
                    transposed: false, // TODO(emilk): have transposed default to true when we can also include per-column metadata
                    ..Default::default()
                };
                println!(
                    "\n{}\n",
                    re_format_arrow::format_record_batch_opts(&chunk, &options)
                );
            } else {
                let options = re_format_arrow::RecordBatchFormatOpts {
                    transposed: false, // TODO(emilk): add cli option for this
                    ..Default::default()
                };
                println!(
                    "\n{}\n",
                    re_format_arrow::format_record_batch_opts(&chunk, &options)
                );
            }
        }

        LogMsg::BlueprintActivationCommand(re_log_types::BlueprintActivationCommand {
            blueprint_id,
            make_active,
            make_default,
        }) => {
            println!(
                "BlueprintActivationCommand({blueprint_id}, make_active: {make_active}, make_default: {make_default})"
            );
        }
    }

    Ok(())
}
