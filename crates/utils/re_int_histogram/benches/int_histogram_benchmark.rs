#![expect(clippy::cast_possible_wrap)] // u64 -> i64 is fine

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{Criterion, criterion_group, criterion_main};

// ----------------

#[cfg(not(debug_assertions))]
const COUNT: u64 = 100_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const COUNT: u64 = 1;

const SPACING: i64 = 1_000_000;

// ----------------

criterion_group!(benches, btree, int_histogram,);
criterion_main!(benches);

// ----------------------------------------------------------------------------

/// Baseline for performance and memory benchmarks
#[derive(Default)]
pub struct BTreeInt64Histogram {
    map: std::collections::BTreeMap<i64, u32>,
}

impl BTreeInt64Histogram {
    pub fn increment(&mut self, key: i64, inc: u32) {
        *self.map.entry(key).or_default() += inc;
    }

    pub fn range(
        &self,
        range: impl std::ops::RangeBounds<i64>,
        _cutoff_size: u64,
    ) -> impl Iterator<Item = (&i64, &u32)> {
        self.map.range(range)
    }
}

/// Baselines
fn btree(c: &mut Criterion) {
    fn create(num_elements: i64, sparseness: i64) -> BTreeInt64Histogram {
        let mut histogram = BTreeInt64Histogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("btree");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPACING));
        });
        let dense = create(COUNT as _, 1);
        group.bench_function("iter_all_dense", |b| {
            b.iter(|| dense.range(.., 1).count());
        });
        let sparse = create(COUNT as _, SPACING);
        group.bench_function("iter_all_sparse", |b| {
            b.iter(|| sparse.range(.., 1).count());
        });
    }
}

fn int_histogram(c: &mut Criterion) {
    use re_int_histogram::Int64Histogram;

    fn create(num_elements: i64, sparseness: i64) -> Int64Histogram {
        let mut histogram = Int64Histogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("int_histogram");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPACING));
        });
        let dense = create(COUNT as _, 1);
        group.bench_function("iter_all_dense", |b| {
            b.iter(|| dense.range(.., 1).count());
        });
        let sparse = create(COUNT as _, SPACING);
        group.bench_function("iter_all_sparse", |b| {
            b.iter(|| sparse.range(.., 1).count());
        });
        let dense = create(COUNT as _, 1);
        group.bench_function("iter_some_dense", |b| {
            b.iter(|| dense.range(.., 1_000).count());
        });
        let sparse = create(COUNT as _, SPACING);
        group.bench_function("iter_some_sparse", |b| {
            b.iter(|| sparse.range(.., 1_000 * SPACING as u64).count());
        });
    }
}
