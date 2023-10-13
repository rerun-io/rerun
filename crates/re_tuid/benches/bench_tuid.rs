use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tuid(c: &mut Criterion) {
    let mut group = c.benchmark_group("tuid");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("Tuid::random", |b| {
        b.iter(|| criterion::black_box(re_tuid::Tuid::random()));
    });
}

#[cfg(feature = "arrow")]
fn bench_arrow(c: &mut Criterion) {
    use arrow2::array::Array;

    {
        let mut group = c.benchmark_group("arrow/serialize");
        group.throughput(criterion::Throughput::Elements(1));

        let tuid = re_tuid::Tuid::random();

        group.bench_function("arrow2", |b| {
            b.iter(|| {
                let data: Box<dyn Array> = tuid.as_arrow();
                criterion::black_box(data)
            });
        });
    }

    {
        let mut group = c.benchmark_group("arrow/deserialize");
        group.throughput(criterion::Throughput::Elements(1));

        let data: Box<dyn Array> = re_tuid::Tuid::random().as_arrow();

        group.bench_function("arrow2", |b| {
            b.iter(|| {
                let tuid = re_tuid::Tuid::from_arrow(data.as_ref()).unwrap();
                criterion::black_box(tuid)
            });
        });
    }
}

criterion_group!(benches, bench_tuid, bench_arrow);
criterion_main!(benches);
