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

criterion_group!(benches, insert_btree, insert_better, insert_binary);
criterion_main!(benches);

// ----------------

/// Baselines
fn insert_btree(c: &mut Criterion) {
    use re_int_histogram::BTreeeIntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> BTreeeIntHistogram {
        let mut histogram = BTreeeIntHistogram::default();
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

fn insert_better(c: &mut Criterion) {
    use re_int_histogram::better::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("better");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}

fn insert_binary(c: &mut Criterion) {
    use re_int_histogram::binary::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("binary");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}
