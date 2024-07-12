//! Visual test for timeline density graphs.
//!
//! ```
//! cargo run -p test_data_density_graph
//! ```

use rerun::external::re_log_types::NonMinI64;
use rerun::time::TimeInt;
use rerun::{
    external::{re_chunk_store, re_log},
    RecordingStream,
};

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_test_data_density_graph")
        .spawn_opts(
            &rerun::SpawnOptions {
                wait_for_bind: true,
                extra_env: {
                    use re_chunk_store::ChunkStoreConfig as C;
                    vec![
                        (C::ENV_STORE_ENABLE_CHANGELOG.into(), "false".into()),
                        (C::ENV_CHUNK_MAX_BYTES.into(), u64::MAX.to_string()),
                        (C::ENV_CHUNK_MAX_ROWS.into(), u64::MAX.to_string()),
                        (
                            C::ENV_CHUNK_MAX_ROWS_IF_UNSORTED.into(),
                            u64::MAX.to_string(),
                        ),
                    ]
                },
                ..Default::default()
            },
            rerun::default_flush_timeout(),
        )?;

    run(&rec)
}

fn run(rec: &RecordingStream) -> anyhow::Result<()> {
    const DESCRIPTION: &str = "\
    Logs different kinds of chunks to exercise different code paths for the density graphs in the time panel:

    - Many small chunks
    - A few large sorted chunks
    - A few large unsorted chunks
    ";
    rec.log_static(
        "description",
        &rerun::TextDocument::from_markdown(DESCRIPTION),
    )?;

    log(rec, "/small", 100, 100, true, 0)?;
    log(rec, "/large", 5, 2000, true, 0)?;
    log(rec, "/large-unsorted", 5, 2000, false, 0)?;
    log(rec, "/gap", 2, 5000, true, 50000)?;

    Ok(())
}

fn log(
    rec: &RecordingStream,
    entity_path: &str,
    num_chunks: i64,
    num_rows_per_chunk: i64,
    sorted: bool,
    first_tick: i64,
) -> anyhow::Result<()> {
    // TODO(jprochazk): unsorted chunk
    let _ = sorted;

    let entity_path = rerun::EntityPath::parse_strict(entity_path)?;

    // log points
    rec.record_chunk_raw(
        rerun::log::Chunk::builder(entity_path.clone())
            .with_archetype(
                rerun::log::RowId::new(),
                rerun::TimePoint::default()
                    .with(
                        rerun::Timeline::log_time(),
                        rerun::time::TimeInt::from_milliseconds(NonMinI64::ZERO),
                    )
                    .with(rerun::Timeline::log_tick(), 0),
                &rerun::Points3D::new(rerun::demo_util::grid(
                    (-10.0, -10.0, -10.0).into(),
                    (10.0, 10.0, 10.0).into(),
                    10,
                )),
            )
            .build()?,
    );

    let rotation = |chunk_idx: i64, row_idx: i64, tick: i64| {
        let timepoint = rerun::TimePoint::default()
            .with(
                rerun::Timeline::log_time(),
                TimeInt::from_milliseconds(tick.try_into().unwrap_or_default()),
            )
            .with(rerun::Timeline::log_tick(), tick);

        let angle_deg = ((chunk_idx as f32 + 1.0) * (row_idx as f32) / 64.0) % 360.0;
        let transform = rerun::Transform3D::from_rotation(rerun::Rotation3D::AxisAngle(
            ((0.0, 0.0, 1.0), rerun::Angle::Degrees(angle_deg)).into(),
        ));

        (timepoint, transform)
    };

    let mut tick = first_tick;
    for chunk_idx in 0..num_chunks {
        let mut chunk = rerun::log::Chunk::builder(entity_path.clone());
        for row_idx in 0..num_rows_per_chunk {
            let (timepoint, transform) = rotation(chunk_idx, row_idx, tick);
            chunk = chunk.with_archetype(rerun::log::RowId::new(), timepoint, &transform);

            tick += 1;
        }
        let mut chunk = chunk.build()?;
        if !sorted {
            chunk.shuffle_random(0xab12_cd34_ef56_0178);
        }
        rec.record_chunk_raw(chunk);

        // add space between chunks
        tick += 100;
    }

    Ok(())
}
