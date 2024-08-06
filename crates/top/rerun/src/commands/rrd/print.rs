use anyhow::Context;
use itertools::Itertools as _;

use re_log_types::{LogMsg, SetStoreInfo};
use re_sdk::log::Chunk;
use re_types::SizeBytes as _;

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct PrintCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,

    /// If set, print out table contents.
    #[clap(long, short, default_value_t = false)]
    verbose: bool,

    /// If set, will try to continue in the face of IO and decoding errors.
    #[clap(long, default_value_t = true)]
    best_effort: bool,
}

impl PrintCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrds,
            verbose,
            best_effort,
        } = self;

        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let (rx, _) = read_rrd_streams_from_file_or_stdin(version_policy, path_to_input_rrds);

        let mut is_success = true;

        for res in rx {
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

            if !*best_effort && !is_success {
                break;
            }
        }

        if is_success {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "one or more IO and/or decoding failures (check logs)"
            ))
        }
    }
}

fn print_msg(verbose: bool, msg: LogMsg) -> anyhow::Result<()> {
    match msg {
        LogMsg::SetStoreInfo(msg) => {
            let SetStoreInfo { row_id: _, info } = msg;
            println!("{info:#?}");
        }

        LogMsg::ArrowMsg(_row_id, arrow_msg) => {
            let chunk = Chunk::from_arrow_msg(&arrow_msg).context("skipped corrupt chunk")?;

            if verbose {
                println!("{chunk}");
            } else {
                let column_names = chunk
                    .component_names()
                    .map(|name| name.short_name())
                    .join(" ");

                println!(
                    "Chunk({}) with {} rows ({}) - {:?} - columns: [{column_names}]",
                    chunk.id(),
                    chunk.num_rows(),
                    re_format::format_bytes(chunk.total_size_bytes() as _),
                    chunk.entity_path(),
                );
            }
        }

        LogMsg::BlueprintActivationCommand(re_log_types::BlueprintActivationCommand {
            blueprint_id,
            make_active,
            make_default,
        }) => {
            println!("BlueprintActivationCommand({blueprint_id}, make_active: {make_active}, make_default: {make_default})");
        }
    }

    Ok(())
}
