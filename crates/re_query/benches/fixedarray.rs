use arrow2::array::FixedSizeListArray;
use arrow2::array::Float32Array;
use arrow2::datatypes::DataType;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BatchSize;
use criterion::Criterion;
use criterion::Throughput;
use itertools::Itertools;

const N: usize = 400_000; // roughly a frame of nyud

fn bench(c: &mut Criterion) {
    let data = vec![[42.0, 420.0, 4200.0]; N];
    let arr = {
        let array_flattened = Float32Array::from_vec(data.into_iter().flatten().collect()).boxed();
        FixedSizeListArray::new(
            FixedSizeListArray::default_datatype(DataType::Float32, 3),
            array_flattened,
            None,
        )
    };

    let mut group = c.benchmark_group("fixed_size_list_array");
    group.throughput(Throughput::Elements(N as u64));

    // Test iteration using iter()
    group.bench_function("iter", |b| {
        let mut count = 0usize;
        b.iter_batched(
            || arr.clone(),
            |data| {
                for p in data.iter() {
                    let p = p.unwrap();
                    let mut iter = p
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .values_iter();
                    let x = iter.next().unwrap();
                    let y = iter.next().unwrap();
                    let z = iter.next().unwrap();
                    assert!(iter.next().is_none());
                    assert_eq!(*x, 42.0);
                    assert_eq!(*y, 420.0);
                    assert_eq!(*z, 4200.0);
                    count += 1;
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Test iteration using values_iter()
    group.bench_function("values_iter", |b| {
        let mut count = 0usize;
        b.iter_batched(
            || arr.clone(),
            |data| {
                for p in data.values_iter() {
                    let mut iter = p
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .values_iter();
                    let x = iter.next().unwrap();
                    let y = iter.next().unwrap();
                    let z = iter.next().unwrap();
                    assert!(iter.next().is_none());
                    assert_eq!(*x, 42.0);
                    assert_eq!(*y, 420.0);
                    assert_eq!(*z, 4200.0);
                    count += 1;
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Test iteration manually using chunks()
    group.bench_function("raw_iter", |b| {
        let mut count = 0usize;
        b.iter_batched(
            || arr.clone(),
            |data| {
                for mut iter in &data
                    .values()
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .unwrap()
                    .values_iter()
                    .chunks(3)
                {
                    let x = iter.next().unwrap();
                    let y = iter.next().unwrap();
                    let z = iter.next().unwrap();
                    assert!(iter.next().is_none());
                    assert_eq!(*x, 42.0);
                    assert_eq!(*y, 420.0);
                    assert_eq!(*z, 4200.0);
                    count += 1;
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
