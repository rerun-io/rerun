use criterion::{criterion_group, criterion_main, Criterion};

fn bench_arrow(c: &mut Criterion) {
    use arrow2::array::Array;
    use re_types_core::Loggable as _;

    for elem_count in [1, 1000] {
        {
            let mut group = c.benchmark_group(format!("arrow/serialize/elem_count={elem_count}"));
            group.throughput(criterion::Throughput::Elements(elem_count));

            let tuids = vec![re_tuid::Tuid::new(); elem_count as usize];

            group.bench_function("arrow2", |b| {
                b.iter(|| {
                    let data: Box<dyn Array> = re_tuid::Tuid::to_arrow(tuids.clone()).unwrap();
                    criterion::black_box(data)
                });
            });
        }

        {
            let mut group = c.benchmark_group(format!("arrow/deserialize/elem_count={elem_count}"));
            group.throughput(criterion::Throughput::Elements(elem_count));

            let data: Box<dyn Array> =
                re_tuid::Tuid::to_arrow(vec![re_tuid::Tuid::new(); elem_count as usize]).unwrap();

            group.bench_function("arrow2", |b| {
                b.iter(|| {
                    let tuids = re_tuid::Tuid::from_arrow(data.as_ref()).unwrap();
                    criterion::black_box(tuids)
                });
            });
        }
    }
}

criterion_group!(benches, bench_arrow);
criterion_main!(benches);
