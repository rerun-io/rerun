use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::Bencher;
use criterion::Criterion;
use re_chunk_store::ChunkStoreConfig;
use re_entity_db::EntityDb;
use re_log_types::ResolvedTimeRange;
use re_log_types::StoreId;
use re_log_types::StoreKind;
use re_log_types::Timeline;
use re_time_panel::__bench::*;
use re_viewer_context::TimeView;

fn run(
    b: &mut Bencher<'_, WallTime>,
    config: DensityGraphBuilderConfig,
    data_entries: &[ChunkEntry],
) {
    egui::__run_test_ui(|ui| {
        let row_rect = ui.max_rect();
        assert!(row_rect.width() > 100.0 && row_rect.height() > 100.0);

        let mut db = EntityDb::with_store_config(
            StoreId::from_string(StoreKind::Recording, "test".into()),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );
        let entity_path = re_log_types::EntityPath::parse_strict("/data").unwrap();

        for ChunkEntry {
            num_chunks,
            num_rows_per_chunk,
            sorted,
            time_start_ms,
        } in data_entries.iter().copied()
        {
            add_data(
                &mut db,
                &entity_path,
                num_chunks,
                num_rows_per_chunk,
                sorted,
                time_start_ms,
            )
            .unwrap();
        }

        let item = TimePanelItem {
            entity_path,
            component_name: None,
        };

        let times = db.times_per_timeline().get(&Timeline::log_time()).unwrap();
        let time_range = ResolvedTimeRange::new(
            times
                .keys()
                .next()
                .copied()
                .unwrap_or(re_log_types::TimeInt::MIN),
            times
                .keys()
                .rev()
                .next()
                .copied()
                .unwrap_or(re_log_types::TimeInt::MIN),
        );

        let time_ranges_ui = TimeRangesUi::new(
            row_rect.x_range(),
            TimeView {
                min: time_range.min().into(),
                time_spanned: time_range.abs_length() as f64,
            },
            &[time_range],
        );
        let timeline = re_log_types::Timeline::log_time();

        b.iter(|| {
            black_box(build_density_graph(
                ui,
                &time_ranges_ui,
                row_rect,
                &db,
                &item,
                timeline,
                config,
            ));
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
) -> anyhow::Result<()> {
    // empty chunk
    if num_chunks == 0 || num_rows_per_chunk == 0 {
        return Ok(());
    }

    // log points
    db.add_chunk(&Arc::new(
        re_chunk_store::Chunk::builder(entity_path.clone())
            .with_archetype(
                re_chunk_store::RowId::new(),
                re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::log_time(),
                    re_log_types::TimeInt::from_milliseconds(re_log_types::NonMinI64::ZERO),
                ),
                &re_types::archetypes::Points3D::new([(10.0, 10.0, 10.0)]),
            )
            .build()?,
    ))?;

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
            let mut rng = rand::thread_rng();
            use rand::seq::SliceRandom as _;
            log_times.shuffle(&mut rng);
        }

        let components = (0..num_rows_per_chunk).map(|i| {
            let angle_deg = i as f32 % 360.0;
            re_types::archetypes::Transform3D::from_rotation(
                re_types::datatypes::Rotation3D::AxisAngle(
                    (
                        (0.0, 0.0, 1.0),
                        re_types::datatypes::Angle::Degrees(angle_deg),
                    )
                        .into(),
                ),
            )
        });

        let mut chunk = re_chunk_store::Chunk::builder(entity_path.clone());
        for (time, component) in log_times.iter().zip(components) {
            chunk = chunk.with_archetype(
                re_chunk_store::RowId::new(),
                re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::log_time(),
                    re_log_types::TimeInt::from_milliseconds(
                        (*time).try_into().unwrap_or_default(),
                    ),
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
    // never split any chunks
    (
        "split_never",
        DensityGraphBuilderConfig {
            max_total_chunk_events: 0,
            max_events_in_unsorted_chunk: 0,
            max_events_in_sorted_chunk: 0,
        },
    ),
    // always split sorted chunks
    (
        "split_sorted_always",
        DensityGraphBuilderConfig {
            max_total_chunk_events: usize::MAX,
            max_events_in_unsorted_chunk: 0,
            max_events_in_sorted_chunk: usize::MAX,
        },
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
                    run(b, config, &[entry]);
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
                    run(b, config, &[entry]);
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
