// Allow unwrap() in benchmarks
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use itertools::Itertools as _;
use re_chunk::{Chunk, RowId, TimelineName};
use re_chunk_store::{ChunkStore, ChunkStoreHandle, ChunkStoreSubscriber as _, LatestAtQuery};
use re_log_types::{EntityPath, TimeInt, TimeType, Timeline, entity_path};
use re_query::{LatestAtResults, QueryCache, clamped_zip_1x1};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::{Color, Position2D, Text};

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const NUM_FRAMES_POINTS: u32 = 1;
    pub const NUM_POINTS: u32 = 1;
    pub const NUM_FRAMES_STRINGS: u32 = 1;
    pub const NUM_STRINGS: u32 = 1;
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const NUM_FRAMES_POINTS: u32 = 1_000;
    pub const NUM_POINTS: u32 = 1_000;
    pub const NUM_FRAMES_STRINGS: u32 = 1_000;
    pub const NUM_STRINGS: u32 = 1_000;
}

#[expect(clippy::wildcard_imports)]
use self::constants::*;

// ---

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion_group!(
    benches,
    mono_points,
    mono_strings,
    batch_points,
    batch_strings
);
criterion_main!(benches);

// ---

fn mono_points(c: &mut Criterion) {
    // Each mono point gets logged at a different path
    let paths = (0..NUM_POINTS)
        .map(move |point_idx| entity_path!("points", point_idx))
        .collect_vec();
    let msgs = build_points_chunks(&paths, 1);

    {
        let mut group = c.benchmark_group("arrow_mono_points2");
        // Mono-insert is slow -- decrease the sample size
        group.sample_size(10);
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES_POINTS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_chunks(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_mono_points2");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let (caches, _store) = insert_chunks(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_points(&caches, &paths));
        });
    }
}

fn mono_strings(c: &mut Criterion) {
    // Each mono string gets logged at a different path
    let paths = (0..NUM_STRINGS)
        .map(move |string_idx| entity_path!("strings", string_idx))
        .collect_vec();
    let msgs = build_strings_chunks(&paths, 1);

    {
        let mut group = c.benchmark_group("arrow_mono_strings2");
        group.sample_size(10);
        group.throughput(criterion::Throughput::Elements(
            (NUM_STRINGS * NUM_FRAMES_STRINGS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_chunks(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_mono_strings2");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let (caches, _store) = insert_chunks(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_strings(&caches, &paths));
        });
    }
}

fn batch_points(c: &mut Criterion) {
    // Batch points are logged together at a single path
    let paths = [EntityPath::from("points")];
    let msgs = build_points_chunks(&paths, NUM_POINTS as _);

    {
        let mut group = c.benchmark_group("arrow_batch_points2");
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES_POINTS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_chunks(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_batch_points2");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let (caches, _store) = insert_chunks(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_points(&caches, &paths));
        });
    }
}

fn batch_strings(c: &mut Criterion) {
    // Batch strings are logged together at a single path
    let paths = [EntityPath::from("points")];
    let msgs = build_strings_chunks(&paths, NUM_STRINGS as _);

    {
        let mut group = c.benchmark_group("arrow_batch_strings2");
        group.throughput(criterion::Throughput::Elements(
            (NUM_STRINGS * NUM_FRAMES_STRINGS) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_chunks(msgs.iter()));
        });
    }

    {
        let mut group = c.benchmark_group("arrow_batch_strings2");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let (caches, _store) = insert_chunks(msgs.iter());
        group.bench_function("query", |b| {
            b.iter(|| query_and_visit_strings(&caches, &paths));
        });
    }
}

// --- Helpers ---

pub fn build_some_point2d(len: usize) -> Vec<Position2D> {
    use rand::Rng as _;
    let mut rng = rand::rng();

    (0..len)
        .map(|_| Position2D::new(rng.random_range(0.0..10.0), rng.random_range(0.0..10.0)))
        .collect()
}

/// Create `len` dummy colors
pub fn build_some_colors(len: usize) -> Vec<Color> {
    (0..len).map(|i| Color::from(i as u32)).collect()
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`re_log_types::TimePoint`].
pub fn build_frame_nr(frame_nr: TimeInt) -> (Timeline, TimeInt) {
    (Timeline::new("frame_nr", TimeType::Sequence), frame_nr)
}

pub fn build_some_strings(len: usize) -> Vec<Text> {
    use rand::Rng as _;
    let mut rng = rand::rng();

    (0..len)
        .map(|_| {
            let ilen: usize = rng.random_range(0..100);
            let s: String = rand::rng()
                .sample_iter(&rand::distr::Alphanumeric)
                .take(ilen)
                .map(char::from)
                .collect();
            Text::from(s)
        })
        .collect()
}

fn build_points_chunks(paths: &[EntityPath], num_points: usize) -> Vec<Arc<Chunk>> {
    paths
        .iter()
        .map(|path| {
            let mut builder = Chunk::builder(path.clone());
            for frame_idx in 0..NUM_FRAMES_POINTS {
                builder = builder.with_archetype(
                    RowId::new(),
                    [build_frame_nr((frame_idx as i64).try_into().unwrap())],
                    &Points2D::new(build_some_point2d(num_points))
                        .with_colors(build_some_colors(num_points)),
                );
            }
            Arc::new(builder.build().unwrap())
        })
        .collect()
}

fn build_strings_chunks(paths: &[EntityPath], num_strings: usize) -> Vec<Arc<Chunk>> {
    paths
        .iter()
        .map(|path| {
            let mut builder = Chunk::builder(path.clone());
            for frame_idx in 0..NUM_FRAMES_POINTS {
                builder = builder.with_archetype(
                    RowId::new(),
                    [build_frame_nr((frame_idx as i64).try_into().unwrap())],
                    // We still need to create points because they are the primary for the
                    // archetype query we want to do. We won't actually deserialize the points
                    // during the query -- we just need it for the primary keys.
                    &Points2D::new(build_some_point2d(num_strings))
                        .with_labels(build_some_strings(num_strings)),
                );
            }
            Arc::new(builder.build().unwrap())
        })
        .collect()
}

fn insert_chunks<'a>(msgs: impl Iterator<Item = &'a Arc<Chunk>>) -> (QueryCache, ChunkStoreHandle) {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    {
        let mut store = store.write();
        msgs.for_each(|chunk| {
            caches.on_events(&store.insert_chunk(chunk).unwrap());
        });
    }

    (caches, store)
}

struct SavePoint {
    _pos: Position2D,
    _color: Option<Color>,
}

fn query_and_visit_points(caches: &QueryCache, paths: &[EntityPath]) -> Vec<SavePoint> {
    let timeline_frame_nr = TimelineName::new("frame_nr");
    let query = LatestAtQuery::new(timeline_frame_nr, NUM_FRAMES_POINTS as i64 / 2);

    let mut ret = Vec::with_capacity(NUM_POINTS as _);

    // TODO(jleibs): Add Radius once we have support for it in field_types
    for entity_path in paths {
        let results: LatestAtResults =
            caches.latest_at(&query, entity_path, Points2D::all_component_identifiers());

        let points = results
            .component_batch_quiet::<Position2D>(Points2D::descriptor_positions().component)
            .unwrap();
        let colors = results
            .component_batch_quiet::<Color>(Points2D::descriptor_colors().component)
            .unwrap_or_default();
        let color_default_fn = || Color::from(0xFF00FFFF);

        for (point, color) in clamped_zip_1x1(points, colors, color_default_fn) {
            ret.push(SavePoint {
                _pos: point,
                _color: Some(color),
            });
        }
    }
    assert_eq!(NUM_POINTS as usize, ret.len());
    ret
}

struct SaveString {
    _label: Option<Text>,
}

fn query_and_visit_strings(caches: &QueryCache, paths: &[EntityPath]) -> Vec<SaveString> {
    let timeline_frame_nr = TimelineName::new("frame_nr");
    let query = LatestAtQuery::new(timeline_frame_nr, NUM_FRAMES_STRINGS as i64 / 2);

    let mut strings = Vec::with_capacity(NUM_STRINGS as _);

    let component_points = Points2D::descriptor_positions().component;
    let component_labels = Points2D::descriptor_labels().component;

    for entity_path in paths {
        let results: LatestAtResults =
            caches.latest_at(&query, entity_path, [component_points, component_labels]);

        let points = results
            .component_batch_quiet::<Position2D>(component_points)
            .unwrap();
        let labels = results
            .component_batch_quiet::<Text>(component_labels)
            .unwrap_or_default();
        let label_default_fn = || Text(String::new().into());

        for (_point, label) in clamped_zip_1x1(points, labels, label_default_fn) {
            strings.push(SaveString {
                _label: Some(label),
            });
        }
    }
    assert_eq!(NUM_STRINGS as usize, strings.len());
    criterion::black_box(strings)
}
