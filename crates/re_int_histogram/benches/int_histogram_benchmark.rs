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

criterion_group!(
    benches,
    insert_btree,
    insert_tree2,
    insert_tree8,
    insert_tree16
);
criterion_main!(benches);

// ----------------

/// Baselines
fn insert_btree(c: &mut Criterion) {
    use re_int_histogram::BTreeeInt64Histogram;

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

fn insert_tree2(c: &mut Criterion) {
    use re_int_histogram::tree2::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("tree2");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}

fn insert_tree8(c: &mut Criterion) {
    use re_int_histogram::tree8::Int64Histogram;

    fn create(num_elements: i64, sparseness: i64) -> Int64Histogram {
        let mut histogram = Int64Histogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("tree8");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}

fn insert_tree16(c: &mut Criterion) {
    use re_int_histogram::tree16::IntHistogram;

    fn create(num_elements: i64, sparseness: i64) -> IntHistogram {
        let mut histogram = IntHistogram::default();
        for i in 0..num_elements {
            histogram.increment(i * sparseness, 1);
        }
        histogram
    }

    {
        let mut group = c.benchmark_group("tree16");
        group.throughput(criterion::Throughput::Elements(COUNT));
        group.bench_function("dense_insert", |b| {
            b.iter(|| create(COUNT as _, 1));
        });
        group.bench_function("sparse_insert", |b| {
            b.iter(|| create(COUNT as _, SPARSENESS));
        });
    }
}
