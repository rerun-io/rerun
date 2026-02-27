#![expect(clippy::unwrap_used)] // acceptable for code which is not user-facing

use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{Bencher, Criterion};
use re_chunk_store::ChunkStoreConfig;
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, StoreId, StoreKind, Timeline};
use re_time_panel::__bench::{
    DensityGraphBuilderConfig, TimePanelItem, TimeRangesUi, build_density_graph,
};

fn run(b: &mut Bencher<'_, WallTime>, config: DensityGraphBuilderConfig, entry: ChunkEntry) {
    egui::__run_test_ui(|ui| {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let row_rect = ui.max_rect();
            assert!(row_rect.width() > 100.0 && row_rect.height() > 100.0);

            let enable_viewer_indexes = true;
            let mut db = EntityDb::with_store_config(
                StoreId::new(StoreKind::Recording, "test-app", "test"),
                enable_viewer_indexes,
                ChunkStoreConfig::COMPACTION_DISABLED,
            );
            let entity_path = re_log_types::EntityPath::parse_strict("/data").unwrap();
            let timeline = re_log_types::Timeline::log_time();

            add_data(
                &mut db,
                &entity_path,
                entry.num_chunks,
                entry.num_rows_per_chunk,
                entry.sorted,
                entry.time_start_ms,
                timeline,
            )
            .unwrap();

            let item = TimePanelItem {
                entity_path,
                component: None,
            };

            let time_range = db
                .time_range_for(timeline.name())
                .unwrap_or(AbsoluteTimeRange::EMPTY);
            let time_ranges_ui =
                TimeRangesUi::new(row_rect.x_range(), time_range.into(), &[time_range]);

            b.iter(|| {
                black_box(build_density_graph(
                    ui,
                    &time_ranges_ui,
                    row_rect,
                    &db,
                    &item,
                    timeline.name(),
                    config,
                ));
            });
        });
    });
}

fn add_data(
    db: &mut EntityDb,
    entity_path: &re_log_types::EntityPath,
    num_chunks: i64,
    num_rows_per_chunk: i64,
    sorted: bool,
    time_start_ms: i64,
    timeline: Timeline,
) -> anyhow::Result<()> {
    // empty chunk
    if num_chunks == 0 || num_rows_per_chunk == 0 {
        return Ok(());
    }

    let mut time = time_start_ms;
    for _ in 0..num_chunks {
        let mut log_times = vec![];
        for _ in 0..num_rows_per_chunk {
            time += 1;
            log_times.push(time);
        }
        time += 100;
        log_times.push(time);

        if !sorted {
            use rand::SeedableRng as _;
            use rand::seq::SliceRandom as _;
            let mut rng = rand::rngs::StdRng::seed_from_u64(0xbadf00d);
            log_times.shuffle(&mut rng);
        }

        let components = (0..num_rows_per_chunk).map(|i| {
            let angle_deg = i as f32 % 360.0;
            re_sdk_types::archetypes::Transform3D::from_rotation(
                re_sdk_types::datatypes::RotationAxisAngle {
                    axis: (0.0, 0.0, 1.0).into(),
                    angle: re_sdk_types::datatypes::Angle::from_degrees(angle_deg),
                },
            )
        });

        let mut chunk = re_chunk_store::Chunk::builder(entity_path.clone());

        // points
        chunk = chunk.with_archetype(
            re_chunk_store::RowId::new(),
            re_log_types::TimePoint::default().with(
                timeline,
                re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::ZERO),
            ),
            &re_sdk_types::archetypes::Points3D::new([(10.0, 10.0, 10.0)]),
        );

        // transforms
        for (time, component) in log_times.iter().zip(components) {
            chunk = chunk.with_archetype(
                re_chunk_store::RowId::new(),
                re_log_types::TimePoint::default().with(
                    timeline,
                    re_log_types::TimeInt::from_millis((*time).try_into().unwrap_or_default()),
                ),
                &component,
            );
        }

        db.add_chunk(&Arc::new(chunk.build()?))?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct ChunkEntry {
    num_chunks: i64,
    num_rows_per_chunk: i64,
    sorted: bool,
    time_start_ms: i64,
}

const fn single_chunk(num_rows_per_chunk: i64, sorted: bool) -> ChunkEntry {
    ChunkEntry {
        num_chunks: 1,
        num_rows_per_chunk,
        sorted,
        time_start_ms: 0,
    }
}

const fn many_chunks(num_chunks: i64, num_rows_per_chunk: i64) -> ChunkEntry {
    ChunkEntry {
        num_chunks,
        num_rows_per_chunk,
        sorted: true,
        time_start_ms: 0,
    }
}

const SCENARIOS: [(&str, DensityGraphBuilderConfig); 2] = [
    (
        "split_never",
        DensityGraphBuilderConfig::NEVER_SHOW_INDIVIDUAL_EVENTS,
    ),
    (
        "split_all",
        DensityGraphBuilderConfig::ALWAYS_SPLIT_ALL_CHUNKS,
    ),
];

fn bench_single_chunks(c: &mut Criterion) {
    for (name, config) in SCENARIOS {
        let mut group = c.benchmark_group(format!("single_chunks/{name}"));

        let sizes = [0, 1, 10, 100, 1000, 10000, 100000];
        for size in sizes {
            for sorted in [true, false] {
                let id = if sorted {
                    format!("{size}/sorted")
                } else {
                    format!("{size}/unsorted")
                };
                group.bench_with_input(id, &single_chunk(size, sorted), |b, &entry| {
                    run(b, config, entry);
                });
            }
        }
    }
}

fn bench_many_chunks(c: &mut Criterion) {
    for (name, config) in SCENARIOS {
        let mut group = c.benchmark_group(format!("many_chunks/{name}"));

        let sizes = [(100, 0), (100, 1), (100, 10), (100, 100), (100, 1000)];
        for (num_chunks, num_rows_per_chunk) in sizes {
            group.bench_with_input(
                format!("{num_chunks}x{num_rows_per_chunk}"),
                &many_chunks(num_chunks, num_rows_per_chunk),
                |b, &entry| {
                    run(b, config, entry);
                },
            );
        }
    }
}

fn main() {
    // More noisy results, but benchmark ends a lot sooner.
    let mut criterion = Criterion::default()
        .configure_from_args()
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_secs(1))
        .sample_size(10)
        .noise_threshold(0.05);

    bench_single_chunks(&mut criterion);
    bench_many_chunks(&mut criterion);

    criterion.final_summary();
}
