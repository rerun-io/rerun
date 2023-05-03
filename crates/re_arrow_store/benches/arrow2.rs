//! Keeping track of performance issues/regressions in `arrow2` that directly affect us.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use arrow2::array::{Array, PrimitiveArray, StructArray, UnionArray};
use criterion::{criterion_group, Criterion};
use itertools::Itertools;
use re_log_types::{
    component_types::{InstanceKey, Point2D, Rect2D},
    datagen::{build_some_instances, build_some_point2d, build_some_rects},
    external::arrow2_convert::serialize::TryIntoArrow,
    DataCell, SerializableComponent, SizeBytes as _,
};

// ---

criterion_group!(benches, erased_clone, estimated_size_bytes);

#[cfg(not(feature = "core_benchmarks_only"))]
criterion::criterion_main!(benches);

// Don't run these benchmarks on CI: they measure the performance of third-party libraries.
#[cfg(feature = "core_benchmarks_only")]
fn main() {}

// ---

#[cfg(not(debug_assertions))]
const NUM_ROWS: usize = 10_000;
#[cfg(not(debug_assertions))]
const NUM_INSTANCES: usize = 100;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_ROWS: usize = 1;
#[cfg(debug_assertions)]
const NUM_INSTANCES: usize = 1;

// ---

#[derive(Debug, Clone, Copy)]
enum ArrayKind {
    /// E.g. an array of `InstanceKey`.
    Primitive,

    /// E.g. an array of `Point2D`.
    Struct,

    /// E.g. an array of `Rect2D`.
    StructLarge,
}

impl std::fmt::Display for ArrayKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ArrayKind::Primitive => "primitive",
            ArrayKind::Struct => "struct",
            ArrayKind::StructLarge => "struct_large",
        })
    }
}

fn erased_clone(c: &mut Criterion) {
    let kind = [
        ArrayKind::Primitive,
        ArrayKind::Struct,
        ArrayKind::StructLarge,
    ];

    for kind in kind {
        let mut group = c.benchmark_group(format!(
            "arrow2/size_bytes/{kind}/rows={NUM_ROWS}/instances={NUM_INSTANCES}"
        ));
        group.throughput(criterion::Throughput::Elements(NUM_ROWS as _));

        match kind {
            ArrayKind::Primitive => {
                let data = build_some_instances(NUM_INSTANCES);
                bench_arrow(&mut group, data.as_slice());
                bench_native(&mut group, data.as_slice());
            }
            ArrayKind::Struct => {
                let data = build_some_point2d(NUM_INSTANCES);
                bench_arrow(&mut group, data.as_slice());
                bench_native(&mut group, data.as_slice());
            }
            ArrayKind::StructLarge => {
                let data = build_some_rects(NUM_INSTANCES);
                bench_arrow(&mut group, data.as_slice());
                bench_native(&mut group, data.as_slice());
            }
        }
    }

    // TODO(cmc): Use cells once `cell.size_bytes()` has landed (#1727)
    fn bench_arrow<T: SerializableComponent>(
        group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
        data: &[T],
    ) {
        let arrays: Vec<Box<dyn Array>> = (0..NUM_ROWS)
            .map(|_| TryIntoArrow::try_into_arrow(data).unwrap())
            .collect_vec();

        let total_size_bytes = arrays
            .iter()
            .map(|array| array.total_size_bytes())
            .sum::<u64>();
        assert!(total_size_bytes as usize >= NUM_ROWS * NUM_INSTANCES * std::mem::size_of::<T>());

        group.bench_function("array", |b| {
            b.iter(|| {
                let sz = arrays
                    .iter()
                    .map(|array| array.total_size_bytes())
                    .sum::<u64>();
                assert_eq!(total_size_bytes, sz);
                sz
            });
        });
    }

    fn bench_native<T: Clone>(
        group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
        data: &[T],
    ) {
        let vecs = (0..NUM_ROWS).map(|_| data.to_vec()).collect_vec();

        let total_size_bytes = vecs
            .iter()
            .map(|vec| std::mem::size_of_val(vec.as_slice()) as u64)
            .sum::<u64>();
        assert!(total_size_bytes as usize >= NUM_ROWS * NUM_INSTANCES * std::mem::size_of::<T>());

        {
            let vecs = (0..NUM_ROWS).map(|_| data.to_vec()).collect_vec();
            group.bench_function("vec", |b| {
                b.iter(|| {
                    let sz = vecs
                        .iter()
                        .map(|vec| std::mem::size_of_val(vec.as_slice()) as u64)
                        .sum::<u64>();
                    assert_eq!(total_size_bytes, sz);
                    sz
                });
            });
        }

        trait SizeOf {
            fn size_of(&self) -> usize;
        }

        impl<T> SizeOf for Vec<T> {
            fn size_of(&self) -> usize {
                std::mem::size_of_val(self.as_slice())
            }
        }

        {
            let vecs: Vec<Box<dyn SizeOf>> = (0..NUM_ROWS)
                .map(|_| Box::new(data.to_vec()) as Box<dyn SizeOf>)
                .collect_vec();

            group.bench_function("vec/erased", |b| {
                b.iter(|| {
                    let sz = vecs.iter().map(|vec| vec.size_of() as u64).sum::<u64>();
                    assert_eq!(total_size_bytes, sz);
                    sz
                });
            });
        }
    }
}

fn estimated_size_bytes(c: &mut Criterion) {
    let kind = [
        ArrayKind::Primitive,
        ArrayKind::Struct,
        ArrayKind::StructLarge,
    ];

    for kind in kind {
        let mut group = c.benchmark_group(format!(
            "arrow2/erased_clone/{kind}/rows={NUM_ROWS}/instances={NUM_INSTANCES}"
        ));
        group.throughput(criterion::Throughput::Elements(NUM_ROWS as _));

        fn generate_cells(kind: ArrayKind) -> Vec<DataCell> {
            match kind {
                ArrayKind::Primitive => (0..NUM_ROWS)
                    .map(|_| DataCell::from_native(build_some_instances(NUM_INSTANCES).as_slice()))
                    .collect(),
                ArrayKind::Struct => (0..NUM_ROWS)
                    .map(|_| DataCell::from_native(build_some_point2d(NUM_INSTANCES).as_slice()))
                    .collect(),
                ArrayKind::StructLarge => (0..NUM_ROWS)
                    .map(|_| DataCell::from_native(build_some_rects(NUM_INSTANCES).as_slice()))
                    .collect(),
            }
        }

        {
            {
                let cells = generate_cells(kind);
                let total_instances = cells.iter().map(|cell| cell.num_instances()).sum::<u32>();
                assert_eq!(total_instances, (NUM_ROWS * NUM_INSTANCES) as u32);

                group.bench_function("cell/arc_erased", |b| {
                    b.iter(|| {
                        let cells = cells.clone();
                        assert_eq!(
                            total_instances,
                            cells.iter().map(|cell| cell.num_instances()).sum::<u32>()
                        );
                        cells
                    });
                });
            }

            {
                let cells = generate_cells(kind).into_iter().map(Arc::new).collect_vec();
                let total_instances = cells.iter().map(|cell| cell.num_instances()).sum::<u32>();
                assert_eq!(total_instances, (NUM_ROWS * NUM_INSTANCES) as u32);

                group.bench_function("cell/wrapped_in_arc", |b| {
                    b.iter(|| {
                        let cells = cells.clone();
                        assert_eq!(
                            total_instances,
                            cells.iter().map(|cell| cell.num_instances()).sum::<u32>()
                        );
                        cells
                    });
                });
            }

            {
                let cells = generate_cells(kind);
                let arrays = cells.iter().map(|cell| cell.to_arrow()).collect_vec();
                let total_instances = arrays.iter().map(|array| array.len() as u32).sum::<u32>();
                assert_eq!(total_instances, (NUM_ROWS * NUM_INSTANCES) as u32);

                group.bench_function("array", |b| {
                    b.iter(|| {
                        let arrays = arrays.clone();
                        assert_eq!(
                            total_instances,
                            arrays.iter().map(|array| array.len() as u32).sum::<u32>()
                        );
                        arrays
                    });
                });
            }

            match kind {
                ArrayKind::Primitive => {
                    bench_downcast_first::<PrimitiveArray<u64>>(&mut group, kind);
                }
                ArrayKind::Struct => bench_downcast_first::<StructArray>(&mut group, kind),
                ArrayKind::StructLarge => bench_downcast_first::<UnionArray>(&mut group, kind),
            }

            fn bench_downcast_first<T: arrow2::array::Array + Clone>(
                group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
                kind: ArrayKind,
            ) {
                let cells = generate_cells(kind);
                let arrays = cells
                    .iter()
                    .map(|cell| {
                        cell.as_arrow_ref()
                            .as_any()
                            .downcast_ref::<T>()
                            .unwrap()
                            .clone()
                    })
                    .collect_vec();
                let total_instances = arrays.iter().map(|array| array.len() as u32).sum::<u32>();
                assert_eq!(total_instances, (NUM_ROWS * NUM_INSTANCES) as u32);

                group.bench_function("array/downcast_first", |b| {
                    b.iter(|| {
                        let arrays = arrays.clone();
                        assert_eq!(
                            total_instances,
                            arrays.iter().map(|array| array.len() as u32).sum::<u32>()
                        );
                        arrays
                    });
                });
            }
        }

        {
            fn generate_points() -> Vec<Vec<Point2D>> {
                (0..NUM_ROWS)
                    .map(|_| build_some_point2d(NUM_INSTANCES))
                    .collect()
            }

            fn generate_keys() -> Vec<Vec<InstanceKey>> {
                (0..NUM_ROWS)
                    .map(|_| build_some_instances(NUM_INSTANCES))
                    .collect()
            }

            fn generate_rects() -> Vec<Vec<Rect2D>> {
                (0..NUM_ROWS)
                    .map(|_| build_some_rects(NUM_INSTANCES))
                    .collect()
            }

            match kind {
                ArrayKind::Primitive => bench_std(&mut group, generate_keys()),
                ArrayKind::Struct => bench_std(&mut group, generate_points()),
                ArrayKind::StructLarge => bench_std(&mut group, generate_rects()),
            }

            fn bench_std<T: Clone>(
                group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
                data: Vec<Vec<T>>,
            ) {
                {
                    let vecs = data.clone();
                    let total_instances = vecs.iter().map(|vec| vec.len() as u32).sum::<u32>();
                    assert_eq!(total_instances, (NUM_ROWS * NUM_INSTANCES) as u32);

                    group.bench_function("vec/full_copy", |b| {
                        b.iter(|| {
                            let vecs = vecs.clone();
                            assert_eq!(
                                total_instances,
                                vecs.iter().map(|vec| vec.len() as u32).sum::<u32>()
                            );
                            vecs
                        });
                    });
                }

                {
                    let vecs = data.into_iter().map(Arc::new).collect_vec();
                    let total_instances = vecs.iter().map(|vec| vec.len() as u32).sum::<u32>();
                    assert_eq!(total_instances, (NUM_ROWS * NUM_INSTANCES) as u32);

                    group.bench_function("vec/wrapped_in_arc", |b| {
                        b.iter(|| {
                            let vecs = vecs.clone();
                            assert_eq!(
                                total_instances,
                                vecs.iter().map(|vec| vec.len() as u32).sum::<u32>()
                            );
                            vecs
                        });
                    });
                }
            }
        }
    }
}
