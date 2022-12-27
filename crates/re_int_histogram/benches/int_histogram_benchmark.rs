#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};

// ----------------

#[cfg(not(debug_assertions))]
const COUNT: u64 = 100_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const COUNT: u64 = 1;

const SPARSENESS: i64 = 1_000_000;

// ----------------

criterion_group!(benches, insert_btree, insert_tree,);
criterion_main!(benches);

// ----------------------------------------------------------------------------

/// Baseline for performance and memory benchmarks
#[derive(Default)]
pub struct BTreeeInt64Histogram {
    map: std::collections::BTreeMap<i64, u32>,
}
impl BTreeeInt64Histogram {
    pub fn increment(&mut self, key: i64, inc: u32) {
        *self.map.entry(key).or_default() += inc;
    }
}

/// Baselines
fn insert_btree(c: &mut Criterion) {
    fn create(num_elements: i64, sparseness: i64) -> BTreeeInt64Histogram {
        let mut histogram = BTreeeInt64Histogram::default();
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
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}

fn insert_tree(c: &mut Criterion) {
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
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}
