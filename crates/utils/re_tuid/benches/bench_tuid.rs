use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tuid(c: &mut Criterion) {
    let mut group = c.benchmark_group("tuid");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("Tuid::new", |b| {
        b.iter(|| criterion::black_box(re_tuid::Tuid::new()));
    });
}

criterion_group!(benches, bench_tuid);
criterion_main!(benches);
