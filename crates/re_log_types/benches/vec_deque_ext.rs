//! Simple benchmark suite to keep track of how the different removal methods for [`VecDeque`]
//! behave in practice.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::VecDeque;

use criterion::{criterion_group, criterion_main, Criterion};

use re_log_types::VecDequeRemovalExt as _;

// ---

criterion_group!(benches, remove, swap_remove, swap_remove_front);
criterion_main!(benches);

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const INITIAL_NUM_ENTRIES: usize = 1;
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const INITIAL_NUM_ENTRIES: usize = 20_000;
}

#[allow(clippy::wildcard_imports)]
use self::constants::*;

// ---

fn remove(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
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
}

fn swap_remove(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
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
}

fn swap_remove_front(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("flat_vec_deque");
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
}

// ---

fn create_prefilled() -> VecDeque<i64> {
    let mut base: VecDeque<i64> = VecDeque::new();
    base.extend(0..INITIAL_NUM_ENTRIES as i64);
    base
}
