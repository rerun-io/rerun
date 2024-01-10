//! High-level benchmark of the CPU-side of our `Points3D` rendering.

use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::{DataRow, EntityPath, RowId, TimeInt, TimePoint, Timeline};
use re_space_view_spatial::{LoadedPoints, Points3DComponentData};
use re_types::{
    archetypes::Points3D,
    components::{ClassId, Color, InstanceKey, KeypointId, Position3D, Radius, Text},
    Loggable as _,
};
use re_viewer_context::Annotations;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion::criterion_main!(benches);
criterion::criterion_group!(benches, bench_points);

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const NUM_POINTS: usize = 10;
    pub const CACHED: &[bool] = &[true];
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const NUM_POINTS: usize = 1_000_000;
    pub const CACHED: &[bool] = &[false, true];
}

#[allow(clippy::wildcard_imports)]
use self::constants::*;

// ---

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
            DataRow::from_archetype(RowId::new(), timepoint, ent_path.clone(), &points).unwrap();
        store.insert_row(&data_row).unwrap();
        store
    };

    let latest_at = LatestAtQuery::latest(timeline);
    let at = latest_at.at;
    let latest_at = re_query_cache::AnyQuery::from(latest_at);
    let annotations = Annotations::missing();

    fn bench_name(cached: bool, name: &str) -> String {
        format!("{name}/cached={cached}")
    }

    for cached in CACHED {
        let mut group = c.benchmark_group("Points3D");
        group.bench_function(bench_name(*cached, "query_archetype"), |b| {
            b.iter(|| {
                re_query_cache::query_archetype_pov1_comp5::<
                    Points3D,
                    Position3D,
                    Color,
                    Radius,
                    Text,
                    KeypointId,
                    ClassId,
                    _,
                >(
                    *cached,
                    &store,
                    &latest_at,
                    &ent_path,
                    |(_, keys, _, _, _, _, _, _)| {
                        assert_eq!(keys.as_slice().len(), NUM_POINTS);
                    },
                )
                .unwrap();
            });
        });
    }

    for cached in CACHED {
        re_query_cache::query_archetype_pov1_comp5::<
            Points3D,
            Position3D,
            Color,
            Radius,
            Text,
            KeypointId,
            ClassId,
            _,
        >(
            *cached,
            &store,
            &latest_at,
            &ent_path,
            |(_, instance_keys, positions, colors, radii, labels, keypoint_ids, class_ids)| {
                let data = Points3DComponentData {
                    instance_keys: instance_keys.as_slice(),
                    positions: positions.as_slice(),
                    colors: colors.as_slice(),
                    radii: radii.as_slice(),
                    labels: labels.as_slice(),
                    keypoint_ids: keypoint_ids
                        .iter()
                        .any(Option::is_some)
                        .then_some(keypoint_ids.as_slice()),
                    class_ids: class_ids
                        .iter()
                        .any(Option::is_some)
                        .then_some(class_ids.as_slice()),
                };
                assert_eq!(data.instance_keys.len(), NUM_POINTS);

                {
                    let mut group = c.benchmark_group("Points3D");
                    group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
                    group.bench_function(bench_name(*cached, "load_all"), |b| {
                        b.iter(|| {
                            let points = LoadedPoints::load(&data, &ent_path, at, &annotations);
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
                    group.bench_function(bench_name(*cached, "load_positions"), |b| {
                        b.iter(|| {
                            let positions = LoadedPoints::load_positions(&data);
                            assert_eq!(positions.len(), NUM_POINTS);
                            positions
                        });
                    });
                }

                {
                    let points = LoadedPoints::load(&data, &ent_path, at, &annotations);

                    let mut group = c.benchmark_group("Points3D");
                    group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
                    group.bench_function(bench_name(*cached, "load_colors"), |b| {
                        b.iter(|| {
                            let colors = LoadedPoints::load_colors(
                                &data,
                                &ent_path,
                                &points.annotation_infos,
                            );
                            assert_eq!(colors.len(), NUM_POINTS);
                            colors
                        });
                    });
                }

                // NOTE: we don't log radii!
                {
                    let mut group = c.benchmark_group("Points3D");
                    group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
                    group.bench_function(bench_name(*cached, "load_radii"), |b| {
                        b.iter(|| {
                            let radii = LoadedPoints::load_radii(&data, &ent_path);
                            assert_eq!(radii.len(), NUM_POINTS);
                            radii
                        });
                    });
                }

                {
                    let mut group = c.benchmark_group("Points3D");
                    group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
                    group.bench_function(bench_name(*cached, "load_picking_ids"), |b| {
                        b.iter(|| {
                            let picking_ids = LoadedPoints::load_picking_ids(&data);
                            assert_eq!(picking_ids.len(), NUM_POINTS);
                            picking_ids
                        });
                    });
                }
            },
        )
        .unwrap();
    }
}
