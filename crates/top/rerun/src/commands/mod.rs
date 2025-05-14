/// Where are we calling [`run`] from?
// TODO(jleibs): Maybe remove call-source all together.
// However, this context of spawn vs direct CLI-invocation still seems
// useful for analytics. We just need to capture the data some other way.
pub enum CallSource {
    /// Called from a command-line-input (the terminal).
    Cli,
}

impl CallSource {
    #[cfg(feature = "native_viewer")]
    fn app_env(&self) -> re_viewer::AppEnvironment {
        match self {
            Self::Cli => re_viewer::AppEnvironment::RerunCli {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
        }
    }
}

// ---

mod entrypoint;
mod rrd;
mod stdio;

#[cfg(feature = "analytics")]
mod analytics;

pub use self::entrypoint::run;
pub use self::rrd::RrdCommands;
pub use self::stdio::read_rrd_streams_from_file_or_stdin;

#[cfg(feature = "analytics")]
pub(crate) use self::analytics::AnalyticsCommands;

// ---

use anyhow::Context as _;
use camino::Utf8PathBuf;
use itertools::Either;
use std::io::Write as _;

/// Saves entity dbs to an RRD file and returns the size of the output file.
fn save_entity_dbs_to_rrd(
    path_to_output_rrd: Option<&Utf8PathBuf>,
    entity_dbs: std::collections::HashMap<re_sdk::StoreId, re_entity_db::EntityDb>,
) -> Result<u64, anyhow::Error> {
    let mut rrd_out = if let Some(path) = path_to_output_rrd {
        Either::Left(std::io::BufWriter::new(
            std::fs::File::create(path).with_context(|| format!("{path:?}"))?,
        ))
    } else {
        Either::Right(std::io::BufWriter::new(std::io::stdout().lock()))
    };

    let messages_rbl = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == re_sdk::StoreKind::Blueprint)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));
    let messages_rrd = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == re_sdk::StoreKind::Recording)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let encoding_options = re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = entity_dbs
        .values()
        .next()
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

    Ok(rrd_out_size)
}
