//! Programmatic access to the `.rrd`/`.rbl` manipulations offered by the `rerun rrd` CLI.
//!
//! Only optimization is available so far.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;

use anyhow::Context as _;
use re_chunk_store::{ChunkStoreConfig, CompactionOptions};
use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreKind};

pub use re_chunk_store::OptimizationProfile;

/// The [`CompactionOptions::is_start_of_gop`] callback, backed by `re_video`.
#[cfg(feature = "video")]
pub(crate) fn gop_detector() -> re_chunk_store::IsStartOfGop {
    std::sync::Arc::new(|data, codec| {
        re_video::is_start_of_gop(data, codec.into()).map_err(|err| anyhow::anyhow!(err))
    })
}

/// Turn an [`OptimizationProfile`] into the options the chunk store expects.
///
/// `fix_keyframe` is left off, since it discards user-supplied keyframe labels rather than
/// validating them.
fn compaction_options(profile: &OptimizationProfile) -> CompactionOptions {
    let mut config = profile.to_chunk_store_config();

    // This is a headless operation: there is nobody to notify, and running subscribers would just
    // (massively) slow us down.
    config.enable_changelog = false;

    CompactionOptions {
        config,
        num_extra_passes: Some(profile.num_extra_passes as usize),
        is_start_of_gop: cfg_select! {
            feature = "video" => profile.gop_batching.then(gop_detector),
            _ => None,
        },
        split_size_ratio: profile.split_size_ratio,
        fix_keyframe: false,
    }
}

/// Optimize an `.rrd`/`.rbl` stream by reshaping its chunks, and write the result to `rrd_out`.
///
/// The entire recording is held in memory for the duration of the call. `rrd_out` is flushed
/// before returning, but nothing is written to it before the input has been read in full.
pub fn optimize(
    rrd_in: impl std::io::Read,
    mut rrd_out: impl std::io::Write,
    profile: &OptimizationProfile,
) -> anyhow::Result<EncodeStats> {
    let options = compaction_options(profile);

    let entity_dbs = load_and_compact(rrd_in, &options)?;

    let stats = encode_entity_dbs(&entity_dbs, &mut rrd_out)?;
    rrd_out.flush().context("couldn't flush output")?;

    Ok(stats)
}

/// Optimize the contents of an existing `.rrd`/`.rbl` file by reshaping its chunks, and write the
/// result to `path_to_output_rrd`.
pub fn optimize_file(
    path_to_input_rrd: impl AsRef<Path>,
    path_to_output_rrd: impl AsRef<Path>,
    profile: &OptimizationProfile,
) -> anyhow::Result<EncodeStats> {
    let path_to_input_rrd = path_to_input_rrd.as_ref();
    let path_to_output_rrd = path_to_output_rrd.as_ref();
    let options = compaction_options(profile);

    let rrd_in = std::fs::File::open(path_to_input_rrd)
        .with_context(|| format!("couldn't open file. File path: {path_to_input_rrd:?}"))?;
    let entity_dbs = load_and_compact(rrd_in, &options)
        .with_context(|| format!("couldn't read file. File path: {path_to_input_rrd:?}"))?;

    let mut rrd_out = std::io::BufWriter::new(
        std::fs::File::create(path_to_output_rrd)
            .with_context(|| format!("couldn't create file. File path: {path_to_output_rrd:?}"))?,
    );
    let stats = encode_entity_dbs(&entity_dbs, &mut rrd_out)?;
    rrd_out.flush().context("couldn't flush output")?;

    Ok(stats)
}

/// Decode a stream into one [`EntityDb`] per store, and compact each of them.
///
/// This is everything both [`optimize`] and [`optimize_file`] do before they write anything.
fn load_and_compact(
    rrd_in: impl std::io::Read,
    options: &CompactionOptions,
) -> anyhow::Result<HashMap<StoreId, EntityDb>> {
    let entity_dbs = read_entity_dbs(rrd_in, &options.config)?;
    compact_entity_dbs(&entity_dbs, options)?;
    Ok(entity_dbs)
}

/// Decode a single `.rrd`/`.rbl` stream into one [`EntityDb`] per store it contains.
fn read_entity_dbs(
    rrd_in: impl std::io::Read,
    store_config: &ChunkStoreConfig,
) -> anyhow::Result<HashMap<StoreId, EntityDb>> {
    let decoder = re_log_encoding::DecoderApp::decode_eager(std::io::BufReader::new(rrd_in))
        .context("couldn't decode input")?;

    let mut entity_dbs: HashMap<StoreId, EntityDb> = Default::default();

    for msg in decoder {
        let msg = msg.context("couldn't decode message")?;
        let db = entity_dbs.entry(msg.store_id().clone()).or_insert_with(|| {
            let enable_viewer_indexes = false; // that would just slow us down for no reason
            EntityDb::with_store_config(
                msg.store_id().clone(),
                enable_viewer_indexes,
                store_config.clone(),
            )
        });
        db.add_log_msg(&msg).context("couldn't index chunk")?;
    }

    Ok(entity_dbs)
}

/// Compact every store in `entity_dbs`, in place.
///
/// The caller must be the sole owner of these stores: this reaches past the storage engine's lock
/// to swap the compacted store in, which is only sound while nothing else can observe them.
pub(crate) fn compact_entity_dbs(
    entity_dbs: &HashMap<StoreId, EntityDb>,
    options: &CompactionOptions,
) -> anyhow::Result<()> {
    let now = std::time::Instant::now();

    let num_physical_chunks = || {
        entity_dbs
            .values()
            .map(|db| db.storage_engine().store().num_physical_chunks() as u64)
            .sum::<u64>()
    };

    let num_chunks_before = num_physical_chunks();

    for db in entity_dbs.values() {
        // Safety: we are the only owners of that data, it's fine.
        #[expect(unsafe_code)]
        let engine = unsafe { db.storage_engine_raw() };

        let compacted = engine.read().store().compacted(options)?;
        *engine.write().store() = compacted;
    }

    let num_chunks_after = num_physical_chunks();

    let num_chunks_reduction = format!(
        "-{:3.3}%",
        100.0 - num_chunks_after as f64 / (num_chunks_before as f64 + f64::EPSILON) * 100.0
    );

    re_log::info!(
        num_chunks_before, num_chunks_after, num_chunks_reduction, time=?now.elapsed(),
        "compaction completed",
    );

    Ok(())
}

/// What [`optimize`] wrote.
#[derive(Clone, Copy, Debug)]
pub struct EncodeStats {
    /// Number of chunks in the output, blueprint chunks excluded.
    pub num_chunks: u64,

    /// Number of encoded message bytes written.
    ///
    /// This counts the messages only: the stream header and the footer (which holds the RRD
    /// manifest) are written around them, so the output is larger than this.
    pub num_bytes: u64,
}

/// Which version to stamp on a stream holding all of `entity_dbs`.
///
/// A single stream carries a single version, so we take the newest one we saw: a reader that can
/// handle it can handle the older stores too. Stores that never declared a version are ignored,
/// and if none of them did, we claim to be the version that wrote the file.
fn output_version(entity_dbs: &HashMap<StoreId, EntityDb>) -> re_build_info::CrateVersion {
    entity_dbs
        .values()
        .filter_map(|db| db.store_info()?.store_version)
        .max()
        .unwrap_or(re_build_info::CrateVersion::LOCAL)
}

/// Encode every store in `entity_dbs` into a single RRD stream.
pub(crate) fn encode_entity_dbs(
    entity_dbs: &HashMap<StoreId, EntityDb>,
    rrd_out: &mut impl std::io::Write,
) -> anyhow::Result<EncodeStats> {
    re_log::info!("preparing output…");
    let messages_rbl = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Blueprint)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */));

    let mut num_chunks = 0u64;
    let messages_rrd = entity_dbs
        .values()
        .filter(|entity_db| entity_db.store_kind() == StoreKind::Recording)
        .flat_map(|entity_db| entity_db.to_messages(None /* time selection */))
        .inspect(|msg| {
            num_chunks += matches!(msg, Ok(re_log_types::LogMsg::ArrowMsg(_, _))) as u64;
        });

    // TODO(cmc): encoding options should match the original.
    let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let version = output_version(entity_dbs);

    re_log::info!("encoding…");
    let num_bytes = re_log_encoding::Encoder::encode_into(
        version,
        encoding_options,
        // NOTE: We want to make sure all blueprints come first, so that the viewer can immediately
        // set up the viewport correctly.
        std::iter::chain(messages_rbl, messages_rrd),
        rrd_out,
    )
    .context("couldn't encode messages")?;

    Ok(EncodeStats {
        num_chunks,
        num_bytes,
    })
}

#[cfg(test)]
mod tests {
    use re_build_info::CrateVersion;
    use re_chunk::{Chunk, ChunkResult, RowId};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreInfo, StoreSource, TimePoint, Timeline,
    };

    use super::*;

    /// An empty recording whose store info carries `store_version`.
    fn versioned_entity_db(
        recording_id: &str,
        store_version: Option<CrateVersion>,
    ) -> anyhow::Result<(StoreId, EntityDb)> {
        let store_id = StoreId::recording("rerun_example_optimize", recording_id);
        let mut db = EntityDb::new(store_id.clone());
        db.add_log_msg(&LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo {
                store_id: store_id.clone(),
                store_source: StoreSource::Unknown,
                store_version,
            },
        }))?;
        Ok((store_id, db))
    }

    /// Merging recordings of different vintages should stamp the output with the newest of them,
    /// no matter which store we happen to walk first, and no matter which ones carry a version.
    #[test]
    fn output_version_is_the_newest_of_all_stores() -> anyhow::Result<()> {
        let newest = CrateVersion::new(0, 42, 0);

        let mut entity_dbs = HashMap::new();
        for (recording_id, version) in [
            ("a_unversioned", None),
            ("b_newest", Some(newest)),
            ("c_older", Some(CrateVersion::new(0, 41, 0))),
        ] {
            let (store_id, db) = versioned_entity_db(recording_id, version)?;
            entity_dbs.insert(store_id, db);
        }

        assert_eq!(newest, output_version(&entity_dbs));

        Ok(())
    }

    /// Writes one single-row chunk per frame: maximally fragmented input for [`optimize`].
    fn write_fragmented_rrd(path: &Path, num_chunks: usize) -> anyhow::Result<()> {
        let store_id = StoreId::random(StoreKind::Recording, "rerun_example_optimize");
        let entity_path = EntityPath::from("/sensor");
        let timeline = Timeline::new_sequence("frame");

        let mut msgs: Vec<ChunkResult<LogMsg>> = vec![Ok(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
        }))];

        for i in 0..num_chunks {
            let frame = i64::try_from(i)?;
            let point = MyPoint::new(i as f32, i as f32);
            let chunk = Chunk::builder(entity_path.clone())
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(timeline, frame)]),
                    (MyPoints::descriptor_points(), &[point]),
                )
                .build()?;
            msgs.push(Ok(LogMsg::ArrowMsg(
                store_id.clone(),
                chunk.to_arrow_msg()?,
            )));
        }

        let mut rrd_out = std::io::BufWriter::new(std::fs::File::create(path)?);
        re_log_encoding::Encoder::encode_into(
            re_build_info::CrateVersion::LOCAL,
            re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED,
            msgs,
            &mut rrd_out,
        )?;
        rrd_out.flush()?;

        Ok(())
    }

    /// Number of chunks and total number of rows in an RRD file.
    fn chunk_stats(path: &Path) -> anyhow::Result<(usize, u64)> {
        chunk_stats_of(std::fs::File::open(path)?)
    }

    /// Number of chunks and total number of rows in an RRD stream.
    fn chunk_stats_of(rrd: impl std::io::Read) -> anyhow::Result<(usize, u64)> {
        let mut num_chunks = 0;
        let mut num_rows = 0;
        for msg in re_log_encoding::DecoderApp::decode_eager(std::io::BufReader::new(rrd))? {
            if let LogMsg::ArrowMsg(_, arrow_msg) = msg? {
                num_chunks += 1;
                num_rows += Chunk::from_arrow_msg(&arrow_msg)?.num_rows() as u64;
            }
        }

        Ok((num_chunks, num_rows))
    }

    /// The reader/writer API should work without ever touching the filesystem.
    #[test]
    fn optimize_works_in_memory() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path_to_input_rrd = dir.path().join("input.rrd");
        write_fragmented_rrd(&path_to_input_rrd, 100)?;
        let input = std::fs::read(&path_to_input_rrd)?;

        let mut output: Vec<u8> = Vec::new();
        let stats = optimize(
            std::io::Cursor::new(&input),
            &mut output,
            &OptimizationProfile::OBJECT_STORE,
        )?;

        let (num_chunks_before, num_rows_before) = chunk_stats_of(input.as_slice())?;
        let (num_chunks_after, num_rows_after) = chunk_stats_of(output.as_slice())?;

        assert_eq!(100, num_chunks_before);
        assert_eq!(num_chunks_after as u64, stats.num_chunks);
        assert!(0 < stats.num_bytes && stats.num_bytes < output.len() as u64);
        assert!(num_chunks_after < num_chunks_before);
        assert_eq!(num_rows_before, num_rows_after);

        Ok(())
    }

    /// [`optimize_file`] reads its input in full before it creates its output, so the two are
    /// allowed to be the very same file — just like `rerun rrd optimize -o out.rrd out.rrd`.
    #[test]
    fn optimize_file_can_work_in_place() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path_to_rrd = dir.path().join("in_place.rrd");

        write_fragmented_rrd(&path_to_rrd, 100)?;
        let (num_chunks_before, num_rows_before) = chunk_stats(&path_to_rrd)?;

        optimize_file(
            &path_to_rrd,
            &path_to_rrd,
            &OptimizationProfile::OBJECT_STORE,
        )?;

        let (num_chunks_after, num_rows_after) = chunk_stats(&path_to_rrd)?;

        assert!(num_chunks_after < num_chunks_before);
        assert_eq!(num_rows_before, num_rows_after);

        Ok(())
    }

    /// A failed read should not leave an output file behind: it is created only after the input has
    /// been read in full.
    #[test]
    fn optimize_file_leaves_no_output_behind_on_a_failed_read() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path_to_input_rrd = dir.path().join("garbage.rrd");
        let path_to_output_rrd = dir.path().join("output.rrd");

        std::fs::write(&path_to_input_rrd, b"this is not an rrd file")?;

        optimize_file(
            &path_to_input_rrd,
            &path_to_output_rrd,
            &OptimizationProfile::OBJECT_STORE,
        )
        .expect_err("garbage input should be an error");

        assert!(!path_to_output_rrd.exists(), "an output file was created");

        Ok(())
    }

    /// A bad output path surfaces as the create error, once the input has been read.
    #[test]
    fn optimize_file_rejects_a_bad_output_path() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path_to_input_rrd = dir.path().join("input.rrd");
        write_fragmented_rrd(&path_to_input_rrd, 1)?;

        let err = optimize_file(
            &path_to_input_rrd,
            dir.path().join("nonexistent").join("output.rrd"),
            &OptimizationProfile::OBJECT_STORE,
        )
        .expect_err("a missing output directory should be an error");

        assert!(
            format!("{err:#}").contains("couldn't create file"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn optimize_file_compacts_and_preserves_rows() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path_to_input_rrd = dir.path().join("input.rrd");
        let path_to_output_rrd = dir.path().join("output.rrd");

        write_fragmented_rrd(&path_to_input_rrd, 100)?;

        let stats = optimize_file(
            &path_to_input_rrd,
            &path_to_output_rrd,
            &OptimizationProfile::OBJECT_STORE,
        )?;

        let (num_chunks_before, num_rows_before) = chunk_stats(&path_to_input_rrd)?;
        let (num_chunks_after, num_rows_after) = chunk_stats(&path_to_output_rrd)?;

        assert_eq!(100, num_chunks_before);
        assert_eq!(num_chunks_after as u64, stats.num_chunks);
        assert!(
            num_chunks_after < num_chunks_before,
            "expected fewer chunks after optimization, got {num_chunks_after} vs {num_chunks_before}"
        );
        assert_eq!(num_rows_before, num_rows_after);

        Ok(())
    }
}
