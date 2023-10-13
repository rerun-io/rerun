use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tuid(c: &mut Criterion) {
    let mut group = c.benchmark_group("tuid");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("Tuid::random", |b| {
        b.iter(|| criterion::black_box(re_tuid::Tuid::random()));
    });
}

#[cfg(feature = "arrow2_convert")]
fn bench_arrow(c: &mut Criterion) {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    {
        let mut group = c.benchmark_group("arrow/serialize");
        group.throughput(criterion::Throughput::Elements(1));

        let tuid = re_tuid::Tuid::random();
        group.bench_function("arrow2_convert", |b| {
            b.iter(|| {
                let data: Box<dyn Array> = vec![tuid].try_into_arrow().unwrap();
                criterion::black_box(data)
            });
        });
    }

    {
        let mut group = c.benchmark_group("arrow/deserialize");
        group.throughput(criterion::Throughput::Elements(1));

        let data: Box<dyn Array> = vec![re_tuid::Tuid::random()].try_into_arrow().unwrap();
        group.bench_function("arrow2_convert", |b| {
            b.iter(|| {
                let tuids: Vec<re_tuid::Tuid> = data.as_ref().try_into_collection().unwrap();
                criterion::black_box(tuids)
            });
        });
    }
}

criterion_group!(benches, bench_tuid, bench_arrow);
criterion_main!(benches);
