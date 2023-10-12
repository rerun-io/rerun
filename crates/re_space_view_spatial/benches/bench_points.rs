//! High-level benchmark of the CPU-side of our `Points3D` rendering.

use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{DataRow, EntityPath, RowId, TimeInt, TimePoint, Timeline};
use re_space_view_spatial::LoadedPoints;
use re_types::{
    archetypes::Points3D,
    components::{Color, InstanceKey, Position3D},
    Loggable as _,
};
use re_viewer_context::Annotations;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion::criterion_main!(benches);
criterion::criterion_group!(benches, bench_points);

// ---

#[cfg(not(debug_assertions))]
const NUM_POINTS: usize = 1_000_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_POINTS: usize = 10;

// ---

/// Mimics `examples/python/open_photogrammetry_format/main.py`
fn bench_points(c: &mut criterion::Criterion) {
    let timeline = Timeline::log_time();
    let ent_path = EntityPath::from("points");

    let store = {
        let mut store = DataStore::new(InstanceKey::name(), Default::default());

        let positions = vec![Position3D::new(0.1, 0.2, 0.3); NUM_POINTS];
        let colors = vec![Color::from(0xffffffff); NUM_POINTS];
        let points = Points3D::new(positions).with_colors(colors);
        let mut timepoint = TimePoint::default();
        timepoint.insert(timeline, TimeInt::from_seconds(0));
        let data_row =
            DataRow::from_archetype(RowId::random(), timepoint, ent_path.clone(), &points).unwrap();
        store.insert_row(&data_row).unwrap();
        store
    };

    let latest_at = LatestAtQuery::latest(timeline);
    let annotations = Annotations::missing();

    {
        let mut group = c.benchmark_group("Points3D");
        group.bench_function("query_archetype", |b| {
            b.iter(|| {
                let arch_view =
                    re_query::query_archetype::<Points3D>(&store, &latest_at, &ent_path).unwrap();
                assert_eq!(arch_view.num_instances(), NUM_POINTS);
                arch_view
            });
        });
    }

    let arch_view = re_query::query_archetype::<Points3D>(&store, &latest_at, &ent_path).unwrap();
    assert_eq!(arch_view.num_instances(), NUM_POINTS);

    {
        let mut group = c.benchmark_group("Points3D");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("load_all", |b| {
            b.iter(|| {
                let points =
                    LoadedPoints::load(&arch_view, &ent_path, latest_at.at, &annotations).unwrap();
                assert_eq!(points.positions.len(), NUM_POINTS);
                assert_eq!(points.colors.len(), NUM_POINTS);
                assert_eq!(points.radii.len(), NUM_POINTS); // NOTE: we don't log radii, but we should get a list of defaults!
                points
            });
        });
    }

    {
        let mut group = c.benchmark_group("Points3D");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("load_positions", |b| {
            b.iter(|| {
                let positions = LoadedPoints::load_positions(&arch_view).unwrap();
                assert_eq!(positions.len(), NUM_POINTS);
                positions
            });
        });
    }

    {
        let points = LoadedPoints::load(&arch_view, &ent_path, latest_at.at, &annotations).unwrap();

        let mut group = c.benchmark_group("Points3D");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("load_colors", |b| {
            b.iter(|| {
                let colors =
                    LoadedPoints::load_colors(&arch_view, &ent_path, &points.annotation_infos)
                        .unwrap();
                assert_eq!(colors.len(), NUM_POINTS);
                colors
            });
        });
    }

    // NOTE: we don't log radii!
    {
        let mut group = c.benchmark_group("Points3D");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("load_radii", |b| {
            b.iter(|| {
                let radii = LoadedPoints::load_radii(&arch_view, &ent_path).unwrap();
                assert_eq!(radii.len(), NUM_POINTS);
                radii
            });
        });
    }

    {
        let mut group = c.benchmark_group("Points3D");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("load_picking_ids", |b| {
            b.iter(|| {
                let picking_ids = LoadedPoints::load_picking_ids(&arch_view);
                assert_eq!(picking_ids.len(), NUM_POINTS);
                picking_ids
            });
        });
    }
}
