use std::path::{Path, PathBuf};

use anyhow::Context as _;
use itertools::Itertools as _;

use re_log_types::{LogMsg, SetStoreInfo};
use re_sdk::log::Chunk;
use re_types::SizeBytes as _;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct PrintCommand {
    path_to_input_rrds: Vec<String>,

    /// If specified, print out table contents.
    #[clap(long, short, default_value_t = false)]
    verbose: bool,
}

impl PrintCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let path_to_input_rrds = self.path_to_input_rrds.iter().map(PathBuf::from);

        for rrd_path in path_to_input_rrds {
            self.print_rrd(&rrd_path)
                .with_context(|| format!("path: {rrd_path:?}"))?;
        }

        Ok(())
    }
}

impl PrintCommand {
    fn print_rrd(&self, rrd_path: &Path) -> anyhow::Result<()> {
        let rrd_file = std::fs::File::open(rrd_path)?;
        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_file)?;
        println!("Decoded RRD stream v{}\n---", decoder.version());
        for msg in decoder {
            let msg = msg.context("decode rrd message")?;
            match msg {
                LogMsg::SetStoreInfo(msg) => {
                    let SetStoreInfo { row_id: _, info } = msg;
                    println!("{info:#?}");
                }

                LogMsg::ArrowMsg(_row_id, arrow_msg) => {
                    let chunk = match Chunk::from_arrow_msg(&arrow_msg) {
                        Ok(chunk) => chunk,
                        Err(err) => {
                            eprintln!("discarding broken chunk: {err}");
                            continue;
                        }
                    };

                    if self.verbose {
                        println!("{chunk}");
                    } else {
                        let column_names = chunk
                            .component_names()
                            .map(|name| name.short_name())
                            .join(" ");

                        println!(
                            "Chunk with {} rows ({}) - {:?} - columns: [{column_names}]",
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
        }
        Ok(())
    }
}
