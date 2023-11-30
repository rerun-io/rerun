//! High-level benchmark of the CPU-side of our `Points3D` rendering.

use re_arrow_store::{DataStore, LatestAtQuery};
use re_log_types::{DataRow, EntityPath, RowId, TimeInt, TimePoint, Timeline};
use re_space_view_spatial::{LoadedPoints, Points3DComponentData};
use re_types::{
    archetypes::Points3D,
    components::{Color, InstanceKey, Position3D, Radius, Text},
    Loggable as _,
};
use re_viewer_context::Annotations;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion::criterion_main!(benches);
criterion::criterion_group!(benches, bench_points, bench_cached_points);

// TODO: remove the name clash

// ---

#[cfg(not(debug_assertions))]
const NUM_POINTS: usize = 1_000_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_POINTS: usize = 10;

// ---

// TODO: oooh, we need to add a cache version for this!

/// Mimics `examples/python/open_photogrammetry_format/main.py`
fn bench_points(c: &mut criterion::Criterion) {
    let timeline = Timeline::log_time();
    let ent_path = EntityPath::from("points");

    let store = {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            Default::default(),
        );

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

fn bench_cached_points(c: &mut criterion::Criterion) {
    let timeline = Timeline::log_time();
    let ent_path = EntityPath::from("points");

    let store = {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            Default::default(),
        );

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

    let at = LatestAtQuery::latest(timeline).at; // TODO
    let latest_at = LatestAtQuery::latest(timeline);
    let latest_at = re_query_cache::AnyQuery::from(latest_at);
    let annotations = Annotations::missing();

    {
        let mut group = c.benchmark_group("Points3D");
        group.bench_function("query_archetype", |b| {
            b.iter(|| {
                re_query_cache::query_cached_archetype_r1o5::<
                    { Points3D::NUM_COMPONENTS },
                    Points3D,
                    Position3D,
                    Color,
                    Radius,
                    Text,
                    re_types::components::KeypointId,
                    re_types::components::ClassId,
                    _,
                >(&store, &latest_at, &ent_path, |it| {
                    for (_, keys, _, _, _, _, _, _) in it {
                        assert_eq!(keys.len(), NUM_POINTS);
                    }
                });
            });
        });
    }

    re_query_cache::query_cached_archetype_r1o5::<
        { Points3D::NUM_COMPONENTS },
        Points3D,
        Position3D,
        Color,
        Radius,
        Text,
        re_types::components::KeypointId,
        re_types::components::ClassId,
        _,
    >(&store, &latest_at, &ent_path, |it| {
        let (_, instance_keys, positions, colors, radii, labels, keypoint_ids, class_ids) =
            it.next().unwrap();

        let data = Points3DComponentData {
            instance_keys,
            positions,
            colors,
            radii,
            labels,
            keypoint_ids: keypoint_ids
                .iter()
                .any(Option::is_some)
                .then_some(keypoint_ids),
            class_ids: class_ids.iter().any(Option::is_some).then_some(class_ids),
        };
        assert_eq!(data.instance_keys.len(), NUM_POINTS);

        {
            let mut group = c.benchmark_group("Points3D");
            group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
            group.bench_function("load_all", |b| {
                b.iter(|| {
                    let points =
                        LoadedPoints::load_cached(&data, &ent_path, at, &annotations).unwrap();
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
                    let positions = LoadedPoints::load_cached_positions(&data);
                    assert_eq!(positions.len(), NUM_POINTS);
                    positions
                });
            });
        }

        {
            let points = LoadedPoints::load_cached(&data, &ent_path, at, &annotations).unwrap();

            let mut group = c.benchmark_group("Points3D");
            group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
            group.bench_function("load_colors", |b| {
                b.iter(|| {
                    let colors = LoadedPoints::load_cached_colors(
                        &data,
                        &ent_path,
                        &points.annotation_infos,
                    )
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
                    let radii = LoadedPoints::load_cached_radii(&data, &ent_path).unwrap();
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
                    let picking_ids = LoadedPoints::load_cached_picking_ids(&data);
                    assert_eq!(picking_ids.len(), NUM_POINTS);
                    picking_ids
                });
            });
        }
    });
}
