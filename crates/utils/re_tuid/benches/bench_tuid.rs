use criterion::{Criterion, criterion_group, criterion_main};

fn bench_tuid(c: &mut Criterion) {
    let mut group = c.benchmark_group("tuid");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("Tuid::new", |b| {
        b.iter(|| criterion::black_box(re_tuid::Tuid::new()));
    });

    group.throughput(criterion::Throughput::Elements(1_000));
    group.bench_function("Tuid::cmp", |b| {
        use rand::prelude::*;
        let mut ids = (0..2_000).map(|_| re_tuid::Tuid::new()).collect::<Vec<_>>();
        ids.shuffle(&mut rand::rng());
        b.iter(|| criterion::black_box(ids[0..1_000].cmp(&ids[1_000..2_000])));
    });
}

criterion_group!(benches, bench_tuid);
criterion_main!(benches);
