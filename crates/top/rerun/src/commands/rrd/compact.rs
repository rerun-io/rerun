use std::path::PathBuf;

use anyhow::Context as _;

use re_entity_db::EntityDb;
use re_log_types::{LogMsg, StoreId};
use re_sdk::StoreKind;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct CompactCommand {
    #[arg(short = 'i', long = "input", value_name = "src.(rrd|rbl)")]
    path_to_input_rrd: String,

    #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
    path_to_output_rrd: String,
}

impl CompactCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_input_rrd,
            path_to_output_rrd,
        } = self;

        let path_to_input_rrd = PathBuf::from(path_to_input_rrd);
        let path_to_output_rrd = PathBuf::from(path_to_output_rrd);

        let rrd_in = std::fs::File::open(&path_to_input_rrd)
            .with_context(|| format!("{path_to_input_rrd:?}"))?;
        let rrd_in_size = rrd_in.metadata().ok().map(|md| md.len());

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
            src = ?path_to_input_rrd,
            src_size_bytes = %file_size_to_string(rrd_in_size),
            dst = ?path_to_output_rrd,
            max_num_rows = %re_format::format_uint(store_config.chunk_max_rows),
            max_num_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
            "compaction started"
        );

        let now = std::time::Instant::now();

        let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();
        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_in)?;
        let version = decoder.version();
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

        anyhow::ensure!(
            !entity_dbs.is_empty(),
            "no recordings found in rrd/rbl file"
        );

        let mut rrd_out = std::fs::File::create(&path_to_output_rrd)
            .with_context(|| format!("{path_to_output_rrd:?}"))?;

        let messages_rbl: Result<Vec<Vec<LogMsg>>, _> = entity_dbs
            .values()
            .filter(|entity_db| entity_db.store_kind() == StoreKind::Blueprint)
            .map(|entity_db| entity_db.to_messages(None /* time selection */))
            .collect();
        let messages_rbl = messages_rbl?;
        let messages_rbl = messages_rbl.iter().flatten();

        let messages_rrd: Result<Vec<Vec<LogMsg>>, _> = entity_dbs
            .values()
            .filter(|entity_db| entity_db.store_kind() == StoreKind::Recording)
            .map(|entity_db| entity_db.to_messages(None /* time selection */))
            .collect();
        let messages_rrd = messages_rrd?;
        let messages_rrd = messages_rrd.iter().flatten();

        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
        re_log_encoding::encoder::encode(
            version,
            encoding_options,
            // NOTE: We want to make sure all blueprints come first, so that the viewer can immediately
            // set up the viewport correctly.
            messages_rbl.chain(messages_rrd),
            &mut rrd_out,
        )
        .context("Message encode")?;

        let rrd_out_size = rrd_out.metadata().ok().map(|md| md.len());

        let compaction_ratio =
            if let (Some(rrd_in_size), Some(rrd_out_size)) = (rrd_in_size, rrd_out_size) {
                format!(
                    "{:3.3}%",
                    100.0 - rrd_out_size as f64 / (rrd_in_size as f64 + f64::EPSILON) * 100.0
                )
            } else {
                "N/A".to_owned()
            };

        re_log::info!(
            src = ?path_to_input_rrd,
            src_size_bytes = %file_size_to_string(rrd_in_size),
            dst = ?path_to_output_rrd,
            dst_size_bytes = %file_size_to_string(rrd_out_size),
            time = ?now.elapsed(),
            compaction_ratio,
            "compaction finished"
        );

        Ok(())
    }
}
