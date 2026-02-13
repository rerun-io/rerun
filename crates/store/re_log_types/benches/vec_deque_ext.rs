#![expect(clippy::cast_possible_wrap)]

//! Simple benchmark suite to keep track of how the different removal methods for [`VecDeque`]
//! behave in practice.

use std::collections::VecDeque;

use criterion::{Criterion, criterion_group, criterion_main};
use itertools::Itertools as _;
use re_log_types::{VecDequeInsertionExt as _, VecDequeRemovalExt as _};

// ---

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion_group!(
    benches,
    insert_many,
    remove_range,
    remove,
    swap_remove,
    swap_remove_front,
);
criterion_main!(benches);

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const INITIAL_NUM_ENTRIES: usize = 1;
    pub const NUM_MODIFIED_ELEMENTS: usize = 1;
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const INITIAL_NUM_ENTRIES: usize = 20_000;
    pub const NUM_MODIFIED_ELEMENTS: usize = 1_000;
}

#[expect(clippy::wildcard_imports)]
use self::constants::*;

// ---

fn insert_many(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let inserted = (0..NUM_MODIFIED_ELEMENTS as i64).collect_vec();

    let mut group = c.benchmark_group("vec_deque");
    group.throughput(criterion::Throughput::Elements(inserted.len() as _));

    group.bench_function("insert_many/prefilled/front", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.insert_many(0, inserted.clone().into_iter());
            v
        });
    });

    group.bench_function("insert_many/prefilled/middle", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.insert_many(INITIAL_NUM_ENTRIES / 2, inserted.clone().into_iter());
            v
        });
    });

    group.bench_function("insert_many/prefilled/back", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.insert_many(INITIAL_NUM_ENTRIES, inserted.clone().into_iter());
            v
        });
    });
}

fn remove_range(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group("vec_deque");
    group.throughput(criterion::Throughput::Elements(NUM_MODIFIED_ELEMENTS as _));

    group.bench_function("remove_range/prefilled/front", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.remove_range(0..NUM_MODIFIED_ELEMENTS);
            v
        });
    });

    group.bench_function("remove_range/prefilled/middle", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.remove_range(
                INITIAL_NUM_ENTRIES / 2 - NUM_MODIFIED_ELEMENTS / 2
                    ..INITIAL_NUM_ENTRIES / 2 + NUM_MODIFIED_ELEMENTS / 2,
            );
            v
        });
    });

    group.bench_function("remove_range/prefilled/back", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.remove_range(INITIAL_NUM_ENTRIES - NUM_MODIFIED_ELEMENTS..INITIAL_NUM_ENTRIES);
            v
        });
    });
}

fn remove(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group("vec_deque");
    group.throughput(criterion::Throughput::Elements(1));

    group.bench_function("remove/prefilled/front", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.remove(0);
            v
        });
    });

    group.bench_function("remove/prefilled/middle", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.remove(INITIAL_NUM_ENTRIES / 2);
            v
        });
    });

    group.bench_function("remove/prefilled/back", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.remove(INITIAL_NUM_ENTRIES - 1);
            v
        });
    });
}

fn swap_remove(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group("vec_deque");
    group.throughput(criterion::Throughput::Elements(1));

    group.bench_function("swap_remove/prefilled/front", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.swap_remove(0);
            v
        });
    });

    group.bench_function("swap_remove/prefilled/middle", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.swap_remove(INITIAL_NUM_ENTRIES / 2);
            v
        });
    });

    group.bench_function("swap_remove/prefilled/back", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.swap_remove(INITIAL_NUM_ENTRIES - 1);
            v
        });
    });
}

fn swap_remove_front(c: &mut Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let mut group = c.benchmark_group("vec_deque");
    group.throughput(criterion::Throughput::Elements(1));

    group.bench_function("swap_remove_front/prefilled/front", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.swap_remove_front(0);
            v
        });
    });

    group.bench_function("swap_remove_front/prefilled/middle", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.swap_remove_front(INITIAL_NUM_ENTRIES / 2);
            v
        });
    });

    group.bench_function("swap_remove_front/prefilled/back", |b| {
        let base = create_prefilled();
        b.iter(|| {
            let mut v: VecDeque<i64> = base.clone();
            v.swap_remove_front(INITIAL_NUM_ENTRIES - 1);
            v
        });
    });
}

// ---

fn create_prefilled() -> VecDeque<i64> {
    let mut base: VecDeque<i64> = VecDeque::new();
    base.extend(0..INITIAL_NUM_ENTRIES as i64);
    base
}
