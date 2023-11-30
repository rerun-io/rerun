#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};

use re_query_cache::FlatVecDeque;

// ---

criterion_group!(benches, extend_back, extend_back_with, extend_at, remove_at);
criterion_main!(benches);

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const INITIAL_LEN_PER_ENTRY: usize = 1;
    pub const INITIAL_NUM_ENTRIES: usize = 1;
    pub const ADDED_LEN: usize = 1;
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const INITIAL_LEN_PER_ENTRY: usize = 1000;
    pub const INITIAL_NUM_ENTRIES: usize = 100;
    pub const ADDED_LEN: usize = 1000;
}

#[allow(clippy::wildcard_imports)]
use self::constants::*;

// ---

// TODO: might be wise to bench iter/range too

fn extend_back(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("extend_back/empty", |b| {
            let added = (0..ADDED_LEN as i64).collect::<Vec<_>>();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = FlatVecDeque::new();
                v.extend_back(added.clone());
                v
            });
        });
    }

    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("extend_back/prefilled", |b| {
            let base = create_prefilled();
            let added = (0..ADDED_LEN as i64).collect::<Vec<_>>();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.extend_back(added.clone());
                v
            });
        });
    }
}

fn extend_back_with(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("extend_back_with/empty", |b| {
            let mut added: FlatVecDeque<i64> = FlatVecDeque::new();
            added.extend_back(0..ADDED_LEN as i64);
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = FlatVecDeque::new();
                v.extend_back_with(added.clone());
                v
            });
        });
    }

    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("extend_back_with/prefilled", |b| {
            let base = create_prefilled();
            let mut added: FlatVecDeque<i64> = FlatVecDeque::new();
            added.extend_back(0..ADDED_LEN as i64);
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.extend_back_with(added.clone());
                v
            });
        });
    }
}

fn extend_at(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("extend_at/empty", |b| {
            let added = (0..ADDED_LEN as i64).collect::<Vec<_>>();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = FlatVecDeque::new();
                v.extend_at(0, added.clone().into_iter());
                v
            });
        });
    }

    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("extend_at/prefilled/front", |b| {
            let base = create_prefilled();
            let added = (0..ADDED_LEN as i64).collect::<Vec<_>>();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.extend_at(0, added.clone().into_iter());
                v
            });
        });
        group.bench_function("extend_at/prefilled/middle", |b| {
            let base = create_prefilled();
            let added = (0..ADDED_LEN as i64).collect::<Vec<_>>();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.extend_at(INITIAL_NUM_ENTRIES / 2, added.clone().into_iter());
                v
            });
        });
        group.bench_function("extend_at/prefilled/back", |b| {
            let base = create_prefilled();
            let added = (0..ADDED_LEN as i64).collect::<Vec<_>>();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.extend_at(INITIAL_NUM_ENTRIES, added.clone().into_iter());
                v
            });
        });
    }
}

fn remove_at(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
        group.throughput(criterion::Throughput::Elements((ADDED_LEN) as _));
        group.bench_function("remove_at/prefilled/front", |b| {
            let base = create_prefilled();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.remove_at(0);
                v
            });
        });
        group.bench_function("remove_at/prefilled/middle", |b| {
            let base = create_prefilled();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.remove_at(INITIAL_NUM_ENTRIES / 2);
                v
            });
        });
        group.bench_function("remove_at/prefilled/back", |b| {
            let base = create_prefilled();
            b.iter(|| {
                let mut v: FlatVecDeque<i64> = base.clone();
                v.remove_at(INITIAL_NUM_ENTRIES);
                v
            });
        });
    }
}

// ---

fn create_prefilled() -> FlatVecDeque<i64> {
    let mut base: FlatVecDeque<i64> = FlatVecDeque::new();
    for _ in 0..INITIAL_NUM_ENTRIES {
        base.extend_back(0..INITIAL_LEN_PER_ENTRY as i64);
    }
    base
}
