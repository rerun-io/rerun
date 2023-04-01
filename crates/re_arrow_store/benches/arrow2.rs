//! Keeping track of performance issues/regressions in `arrow2` that directly affect us.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use arrow2::array::{Array, PrimitiveArray, StructArray};
use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use re_log_types::{
    component_types::{InstanceKey, Point2D},
    datagen::{build_some_instances, build_some_point2d},
    DataCell,
};

// ---

criterion_group!(benches, estimated_size_bytes);
criterion_main!(benches);

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

fn estimated_size_bytes(c: &mut Criterion) {
    let kind = ["primitive", "struct"];

    for kind in kind {
        let mut group = c.benchmark_group(format!(
            "arrow2/erased_clone/{kind}/rows={NUM_ROWS}/instances={NUM_INSTANCES}"
        ));
        group.throughput(criterion::Throughput::Elements(NUM_ROWS as _));

        fn generate_cells(kind: &str) -> Vec<DataCell> {
            match kind {
                "primitive" => (0..NUM_ROWS)
                    .map(|_| DataCell::from_native(build_some_instances(NUM_INSTANCES).as_slice()))
                    .collect(),
                "struct" => (0..NUM_ROWS)
                    .map(|_| DataCell::from_native(build_some_point2d(NUM_INSTANCES).as_slice()))
                    .collect(),
                _ => unreachable!(),
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
                let arrays = cells.iter().map(|cell| cell.as_arrow()).collect_vec();
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
                "primitive" => {
                    let cells = generate_cells(kind);
                    let arrays = cells
                        .iter()
                        .map(|cell| {
                            cell.as_arrow_ref()
                                .as_any()
                                .downcast_ref::<PrimitiveArray<u64>>()
                                .unwrap()
                                .clone()
                        })
                        .collect_vec();
                    let total_instances =
                        arrays.iter().map(|array| array.len() as u32).sum::<u32>();
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
                "struct" => {
                    let cells = generate_cells(kind);
                    let arrays = cells
                        .iter()
                        .map(|cell| {
                            cell.as_arrow_ref()
                                .as_any()
                                .downcast_ref::<StructArray>()
                                .unwrap()
                                .clone()
                        })
                        .collect_vec();
                    let total_instances =
                        arrays.iter().map(|array| array.len() as u32).sum::<u32>();
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
                _ => unreachable!(),
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

            match kind {
                "primitive" => bench_std(&mut group, generate_keys()),
                "struct" => bench_std(&mut group, generate_points()),
                _ => unreachable!(),
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
