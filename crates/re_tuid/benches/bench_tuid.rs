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

    for nb_elems in [1, 1000] {
        {
            let mut group = c.benchmark_group(format!("arrow/serialize/nb_elems={nb_elems}"));
            group.throughput(criterion::Throughput::Elements(nb_elems));

            let tuids = vec![re_tuid::Tuid::random(); nb_elems as usize];

            group.bench_function("arrow2", |b| {
                b.iter(|| {
                    let data: Box<dyn Array> = re_tuid::Tuid::to_arrow(tuids.clone());
                    criterion::black_box(data)
                });
            });
        }

        {
            let mut group = c.benchmark_group(format!("arrow/deserialize/nb_elems={nb_elems}"));
            group.throughput(criterion::Throughput::Elements(nb_elems));

            let data: Box<dyn Array> =
                re_tuid::Tuid::to_arrow(vec![re_tuid::Tuid::random(); nb_elems as usize]);

            group.bench_function("arrow2", |b| {
                b.iter(|| {
                    let tuids = re_tuid::Tuid::from_arrow(data.as_ref()).unwrap();
                    criterion::black_box(tuids)
                });
            });
        }
    }
}

criterion_group!(benches, bench_tuid, bench_arrow);
criterion_main!(benches);
