use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use itertools::{izip, Itertools};

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct CompareCommand {
    path_to_rrd1: String,
    path_to_rrd2: String,

    /// If specified, dumps both .rrd files as tables.
    #[clap(long, default_value_t = false)]
    full_dump: bool,
}

impl CompareCommand {
    /// Checks whether two .rrd files are _similar_, i.e. not equal on a byte-level but
    /// functionally equivalent.
    ///
    /// Returns `Ok(())` if they match, or an error containing a detailed diff otherwise.
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            path_to_rrd1,
            path_to_rrd2,
            full_dump,
        } = self;

        re_log::debug!("Comparing {path_to_rrd1:?} to {path_to_rrd2:?}…");

        let path_to_rrd1 = PathBuf::from(path_to_rrd1);
        let path_to_rrd2 = PathBuf::from(path_to_rrd2);

        let (app_id1, chunks1) =
            compute_uber_table(&path_to_rrd1).with_context(|| format!("path: {path_to_rrd1:?}"))?;
        let (app_id2, chunks2) =
            compute_uber_table(&path_to_rrd2).with_context(|| format!("path: {path_to_rrd2:?}"))?;

        if *full_dump {
            println!("{app_id1}");
            for chunk in &chunks1 {
                println!("{chunk}");
            }

            println!("{app_id2}");
            for chunk in &chunks2 {
                println!("{chunk}");
            }
        }

        anyhow::ensure!(
            app_id1 == app_id2,
            "Application IDs do not match: '{app_id1}' vs. '{app_id2}'"
        );

        anyhow::ensure!(
            chunks1.len() == chunks2.len(),
            "Number of Chunks does not match: '{}' vs. '{}'",
            re_format::format_uint(chunks1.len()),
            re_format::format_uint(chunks2.len()),
        );

        for (chunk1, chunk2) in izip!(chunks1, chunks2) {
            anyhow::ensure!(
                re_chunk::Chunk::are_similar(&chunk1, &chunk2),
                "Chunks do not match:\n{}",
                similar_asserts::SimpleDiff::from_str(
                    &format!("{chunk1}"),
                    &format!("{chunk2}"),
                    "got",
                    "expected",
                ),
            );
        }

        re_log::debug!("{path_to_rrd1:?} and {path_to_rrd2:?} are similar enough.");

        Ok(())
    }
}

/// Given a path to an rrd file, builds up a `ChunkStore` and returns its contents a stream of
/// `Chunk`s.
///
/// Fails if there are more than one data recordings present in the rrd file.
fn compute_uber_table(
    path_to_rrd: &Path,
) -> anyhow::Result<(re_log_types::ApplicationId, Vec<Arc<re_chunk::Chunk>>)> {
    use re_entity_db::EntityDb;
    use re_log_types::StoreId;

    let rrd_file = std::fs::File::open(path_to_rrd).context("couldn't open rrd file contents")?;
    let rrd_file = std::io::BufReader::new(rrd_file);

    let mut stores: std::collections::HashMap<StoreId, EntityDb> = Default::default();
    let version_policy = re_log_encoding::VersionPolicy::Error;
    let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_file)?;
    for msg in decoder {
        let msg = msg.context("decode rrd message")?;
        stores
            .entry(msg.store_id().clone())
            .or_insert_with(|| re_entity_db::EntityDb::new(msg.store_id().clone()))
            .add(&msg)
            .context("decode rrd file contents")?;
    }

    let mut stores = stores
        .values()
        .filter(|store| store.store_kind() == re_log_types::StoreKind::Recording)
        .collect_vec();

    anyhow::ensure!(!stores.is_empty(), "no data recording found in rrd file");
    anyhow::ensure!(
        stores.len() == 1,
        "more than one data recording found in rrd file"
    );

    let store = stores.pop().unwrap(); // safe, ensured above
    let engine = store.storage_engine();

    Ok((
        store
            .app_id()
            .cloned()
            .unwrap_or_else(re_log_types::ApplicationId::unknown),
        engine.store().iter_chunks().map(Arc::clone).collect_vec(),
    ))
}
