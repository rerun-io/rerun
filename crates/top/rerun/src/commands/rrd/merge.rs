use std::path::PathBuf;

use anyhow::Context as _;
use itertools::Itertools;

use re_log_types::LogMsg;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct MergeCommand {
    path_to_input_rrds: Vec<String>,

    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: String,
}

impl MergeCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        use re_entity_db::EntityDb;
        use re_log_types::StoreId;

        let Self {
            path_to_input_rrds,
            path_to_output_rrd,
        } = self;

        let path_to_input_rrds = path_to_input_rrds.iter().map(PathBuf::from).collect_vec();
        let path_to_output_rrd = PathBuf::from(path_to_output_rrd);

        let rrds_in: Result<Vec<_>, _> = path_to_input_rrds
            .iter()
            .map(|path_to_input_rrd| {
                std::fs::File::open(path_to_input_rrd)
                    .with_context(|| format!("{path_to_input_rrd:?}"))
            })
            .collect();
        let rrds_in = rrds_in?;

        let rrds_in_size = rrds_in
            .iter()
            .map(|rrd_in| rrd_in.metadata().ok().map(|md| md.len()))
            .sum::<Option<u64>>();

        let file_size_to_string = |size: Option<u64>| {
            size.map_or_else(
                || "<unknown>".to_owned(),
                |size| re_format::format_bytes(size as _),
            )
        };

        use re_chunk_store::ChunkStoreConfig;
        let mut store_config = ChunkStoreConfig::from_env().unwrap_or_default();
        // NOTE: We're doing headless processing, there's no point in running subscribers, it will just
        // (massively) slow us down.
        store_config.enable_changelog = false;

        re_log::info!(
            srcs = ?path_to_input_rrds,
            dst = ?path_to_output_rrd,
            max_num_rows = %re_format::format_uint(store_config.chunk_max_rows),
            max_num_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
            "merge started"
        );

        let now = std::time::Instant::now();

        let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();
        let mut version = None;
        for rrd_in in rrds_in {
            let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
            let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_in)?;
            version = version.max(Some(decoder.version()));
            for msg in decoder {
                let msg = msg.context("decode rrd message")?;
                entity_dbs
                    .entry(msg.store_id().clone())
                    .or_insert_with(|| {
                        re_entity_db::EntityDb::with_store_config(
                            msg.store_id().clone(),
                            store_config.clone(),
                        )
                    })
                    .add(&msg)
                    .context("decode rrd file contents")?;
            }
        }

        anyhow::ensure!(
            !entity_dbs.is_empty(),
            "no recordings found in rrd/rbl files"
        );

        let mut rrd_out = std::fs::File::create(&path_to_output_rrd)
            .with_context(|| format!("{path_to_output_rrd:?}"))?;

        let messages: Result<Vec<Vec<LogMsg>>, _> = entity_dbs
            .into_values()
            .map(|entity_db| entity_db.to_messages(None /* time selection */))
            .collect();
        let messages = messages?;
        let messages = messages.iter().flatten();

        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
        let version = version.unwrap_or(re_build_info::CrateVersion::LOCAL);
        re_log_encoding::encoder::encode(version, encoding_options, messages, &mut rrd_out)
            .context("Message encode")?;

        let rrd_out_size = rrd_out.metadata().ok().map(|md| md.len());

        re_log::info!(
            srcs = ?path_to_input_rrds,
            srcs_size_bytes = %file_size_to_string(rrds_in_size),
            dst = ?path_to_output_rrd,
            dst_size_bytes = %file_size_to_string(rrd_out_size),
            time = ?now.elapsed(),
            "merge finished"
        );

        Ok(())
    }
}
