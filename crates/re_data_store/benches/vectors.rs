//! Keeping track of performance issues/regressions for common vector operations.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, Criterion};

use smallvec::SmallVec;
use tinyvec::TinyVec;

// ---

criterion_group!(benches, sort, split, swap, swap_opt);

criterion::criterion_main!(benches);

// ---

#[cfg(not(debug_assertions))]
const NUM_INSTANCES: usize = 10_000;
#[cfg(not(debug_assertions))]
const SMALLVEC_SIZE: usize = 4;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_INSTANCES: usize = 1;
#[cfg(debug_assertions)]
const SMALLVEC_SIZE: usize = 1;

// --- Benchmarks ---

fn split(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group(format!("vector_ops/split_off/instances={NUM_INSTANCES}"));
    group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

    {
        fn split_off<T: Copy, const N: usize>(
            data: &mut SmallVec<[T; N]>,
            split_idx: usize,
        ) -> SmallVec<[T; N]> {
            if split_idx >= data.len() {
                return SmallVec::default();
            }

            let second_half = SmallVec::from_slice(&data[split_idx..]);
            data.truncate(split_idx);
            second_half
        }

        let data: SmallVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).collect();

        group.bench_function(format!("smallvec/n={SMALLVEC_SIZE}/manual"), |b| {
            b.iter(|| {
                let mut data = data.clone();
                let second_half = split_off(&mut data, NUM_INSTANCES / 2);
                assert_eq!(NUM_INSTANCES, data.len() + second_half.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, second_half[0]);
                (data, second_half)
            });
        });
    }

    {
        let data: TinyVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).collect();

        group.bench_function(format!("tinyvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data = data.clone();
                let second_half = data.split_off(NUM_INSTANCES / 2);
                assert_eq!(NUM_INSTANCES, data.len() + second_half.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, second_half[0]);
                (data, second_half)
            });
        });
    }

    {
        fn split_off<T: Default + Copy, const N: usize>(
            data: &mut TinyVec<[T; N]>,
            split_idx: usize,
        ) -> TinyVec<[T; N]> {
            if split_idx >= data.len() {
                return TinyVec::default();
            }

            let second_half = TinyVec::from(&data[split_idx..]);
            data.truncate(split_idx);
            second_half
        }

        let data: TinyVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).collect();

        group.bench_function(format!("tinyvec/n={SMALLVEC_SIZE}/manual"), |b| {
            b.iter(|| {
                let mut data = data.clone();
                let second_half = split_off(&mut data, NUM_INSTANCES / 2);
                assert_eq!(NUM_INSTANCES, data.len() + second_half.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, second_half[0]);
                (data, second_half)
            });
        });
    }

    {
        let data: Vec<i64> = (0..NUM_INSTANCES as i64).collect();

        group.bench_function("vec", |b| {
            b.iter(|| {
                let mut data = data.clone();
                let second_half = data.split_off(NUM_INSTANCES / 2);
                assert_eq!(NUM_INSTANCES, data.len() + second_half.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, second_half[0]);
                (data, second_half)
            });
        });
    }

    {
        fn split_off<T: Copy>(data: &mut Vec<T>, split_idx: usize) -> Vec<T> {
            if split_idx >= data.len() {
                return Vec::default();
            }

            let second_half = Vec::from(&data[split_idx..]);
            data.truncate(split_idx);
            second_half
        }

        let data: Vec<i64> = (0..NUM_INSTANCES as i64).collect();

        group.bench_function("vec/manual", |b| {
            b.iter(|| {
                let mut data = data.clone();
                let second_half = split_off(&mut data, NUM_INSTANCES / 2);
                assert_eq!(NUM_INSTANCES, data.len() + second_half.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, second_half[0]);
                (data, second_half)
            });
        });
    }
}

fn sort(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group(format!("vector_ops/sort/instances={NUM_INSTANCES}"));
    group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

    {
        let data: SmallVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).rev().collect();

        group.bench_function(format!("smallvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data = data.clone();
                data.sort_unstable();
                assert_eq!(NUM_INSTANCES, data.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, data[NUM_INSTANCES / 2]);
                data
            });
        });
    }

    {
        let data: TinyVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).rev().collect();

        group.bench_function(format!("tinyvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data = data.clone();
                data.sort_unstable();
                assert_eq!(NUM_INSTANCES, data.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, data[NUM_INSTANCES / 2]);
                data
            });
        });
    }

    {
        let data: Vec<i64> = (0..NUM_INSTANCES as i64).rev().collect();

        group.bench_function("vec", |b| {
            b.iter(|| {
                let mut data = data.clone();
                data.sort_unstable();
                assert_eq!(NUM_INSTANCES, data.len());
                assert_eq!(NUM_INSTANCES as i64 / 2, data[NUM_INSTANCES / 2]);
                data
            });
        });
    }
}

fn swap(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group(format!("vector_ops/swap/instances={NUM_INSTANCES}"));
    group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

    {
        let data: SmallVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).collect();
        let swaps: SmallVec<[usize; SMALLVEC_SIZE]> = (0..NUM_INSTANCES).rev().collect();

        group.bench_function(format!("smallvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data1 = data.clone();
                let data2 = data.clone();
                for &swap in &swaps {
                    data1[NUM_INSTANCES - swap - 1] = data2[swap];
                }
                assert_eq!(NUM_INSTANCES, data1.len());
                assert_eq!(NUM_INSTANCES, data2.len());
                assert_eq!(
                    (NUM_INSTANCES as i64 / 2).max(1) - 1,
                    data1[NUM_INSTANCES / 2]
                );
                (data1, data2)
            });
        });
    }

    {
        let data: TinyVec<[i64; SMALLVEC_SIZE]> = (0..NUM_INSTANCES as i64).collect();
        let swaps: TinyVec<[usize; SMALLVEC_SIZE]> = (0..NUM_INSTANCES).rev().collect();

        group.bench_function(format!("tinyvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data1 = data.clone();
                let data2 = data.clone();
                for &swap in &swaps {
                    data1[NUM_INSTANCES - swap - 1] = data2[swap];
                }
                assert_eq!(NUM_INSTANCES, data1.len());
                assert_eq!(NUM_INSTANCES, data2.len());
                assert_eq!(
                    (NUM_INSTANCES as i64 / 2).max(1) - 1,
                    data1[NUM_INSTANCES / 2]
                );
                (data1, data2)
            });
        });
    }

    {
        let data: Vec<i64> = (0..NUM_INSTANCES as i64).collect();
        let swaps: Vec<usize> = (0..NUM_INSTANCES).rev().collect();

        group.bench_function("vec", |b| {
            b.iter(|| {
                let mut data1 = data.clone();
                let data2 = data.clone();
                for &swap in &swaps {
                    data1[NUM_INSTANCES - swap - 1] = data2[swap];
                }
                assert_eq!(NUM_INSTANCES, data1.len());
                assert_eq!(NUM_INSTANCES, data2.len());
                assert_eq!(
                    (NUM_INSTANCES as i64 / 2).max(1) - 1,
                    data1[NUM_INSTANCES / 2]
                );
                (data1, data2)
            });
        });
    }
}

fn swap_opt(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group(format!("vector_ops/swap_opt/instances={NUM_INSTANCES}"));
    group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

    {
        let data: SmallVec<[Option<i64>; SMALLVEC_SIZE]> =
            (0..NUM_INSTANCES as i64).map(Some).collect();
        let swaps: SmallVec<[usize; SMALLVEC_SIZE]> = (0..NUM_INSTANCES).rev().collect();

        group.bench_function(format!("smallvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data1 = data.clone();
                let mut data2 = data.clone();
                for &swap in &swaps {
                    data1[NUM_INSTANCES - swap - 1] = data2[swap].take();
                }
                assert_eq!(NUM_INSTANCES, data1.len());
                assert_eq!(NUM_INSTANCES, data2.len());
                assert_eq!(
                    Some((NUM_INSTANCES as i64 / 2).max(1) - 1),
                    data1[NUM_INSTANCES / 2]
                );
                (data1, data2)
            });
        });
    }

    {
        let data: TinyVec<[Option<i64>; SMALLVEC_SIZE]> =
            (0..NUM_INSTANCES as i64).map(Some).collect();
        let swaps: TinyVec<[usize; SMALLVEC_SIZE]> = (0..NUM_INSTANCES).rev().collect();

        group.bench_function(format!("tinyvec/n={SMALLVEC_SIZE}"), |b| {
            b.iter(|| {
                let mut data1 = data.clone();
                let mut data2 = data.clone();
                for &swap in &swaps {
                    data1[NUM_INSTANCES - swap - 1] = data2[swap].take();
                }
                assert_eq!(NUM_INSTANCES, data1.len());
                assert_eq!(NUM_INSTANCES, data2.len());
                assert_eq!(
                    Some((NUM_INSTANCES as i64 / 2).max(1) - 1),
                    data1[NUM_INSTANCES / 2]
                );
                (data1, data2)
            });
        });
    }

    {
        let data: Vec<Option<i64>> = (0..NUM_INSTANCES as i64).map(Some).collect();
        let swaps: Vec<usize> = (0..NUM_INSTANCES).rev().collect();

        group.bench_function("vec", |b| {
            b.iter(|| {
                let mut data1 = data.clone();
                let mut data2 = data.clone();
                for &swap in &swaps {
                    data1[NUM_INSTANCES - swap - 1] = data2[swap].take();
                }
                assert_eq!(NUM_INSTANCES, data1.len());
                assert_eq!(NUM_INSTANCES, data2.len());
                assert_eq!(
                    Some((NUM_INSTANCES as i64 / 2).max(1) - 1),
                    data1[NUM_INSTANCES / 2]
                );
                (data1, data2)
            });
        });
    }
}
