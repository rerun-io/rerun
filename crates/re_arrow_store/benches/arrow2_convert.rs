//! Keeping track of performance issues/regressions in `arrow2_convert` that directly affect us.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use arrow2::{array::PrimitiveArray, datatypes::PhysicalType, types::PrimitiveType};
use criterion::{criterion_group, Criterion};
use re_log_types::DataCell;
use re_types::{components::InstanceKey, Loggable as _};

// ---

criterion_group!(benches, serialize, deserialize);

#[cfg(not(feature = "core_benchmarks_only"))]
criterion::criterion_main!(benches);

// Don't run these benchmarks on CI: they measure the performance of third-party libraries.
#[cfg(feature = "core_benchmarks_only")]
fn main() {}

// ---

#[cfg(not(debug_assertions))]
const NUM_INSTANCES: usize = 100_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_INSTANCES: usize = 1;

// ---

fn serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!(
        "arrow2_convert/serialize/primitive/instances={NUM_INSTANCES}"
    ));
    group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

    {
        group.bench_function("arrow2_convert", |b| {
            b.iter(|| {
                let cell = DataCell::from_component::<InstanceKey>(0..NUM_INSTANCES as u64);
                assert_eq!(NUM_INSTANCES as u32, cell.num_instances());
                assert_eq!(
                    cell.datatype().to_physical_type(),
                    PhysicalType::Primitive(PrimitiveType::UInt64)
                );
                cell
            });
        });
    }

    {
        group.bench_function("arrow2/from_values", |b| {
            b.iter(|| {
                let values = PrimitiveArray::from_values(0..NUM_INSTANCES as u64).boxed();
                let cell = crate::DataCell::from_arrow(InstanceKey::name(), values);
                assert_eq!(NUM_INSTANCES as u32, cell.num_instances());
                assert_eq!(
                    cell.datatype().to_physical_type(),
                    PhysicalType::Primitive(PrimitiveType::UInt64)
                );
                cell
            });
        });
    }

    {
        group.bench_function("arrow2/from_vec", |b| {
            b.iter(|| {
                // NOTE: We do the `collect()` here on purpose!
                //
                // All of these APIs have to allocate an array under the hood, except `from_vec`
                // which is O(1) (it just unsafely reuses the vec's data pointer).
                // We need to measure the collection in order to have a leveled playing field.
                let values = PrimitiveArray::from_vec((0..NUM_INSTANCES as u64).collect()).boxed();
                let cell = crate::DataCell::from_arrow(InstanceKey::name(), values);
                assert_eq!(NUM_INSTANCES as u32, cell.num_instances());
                assert_eq!(
                    cell.datatype().to_physical_type(),
                    PhysicalType::Primitive(PrimitiveType::UInt64)
                );
                cell
            });
        });
    }
}

fn deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!(
        "arrow2_convert/deserialize/primitive/instances={NUM_INSTANCES}"
    ));
    group.throughput(criterion::Throughput::Elements(NUM_INSTANCES as _));

    let cell = DataCell::from_component::<InstanceKey>(0..NUM_INSTANCES as u64);
    let data = cell.to_arrow();

    {
        group.bench_function("arrow2_convert", |b| {
            b.iter(|| {
                let keys: Vec<InstanceKey> = InstanceKey::from_arrow(data.as_ref()).unwrap();
                assert_eq!(NUM_INSTANCES, keys.len());
                assert_eq!(
                    InstanceKey(NUM_INSTANCES as u64 / 2),
                    keys[NUM_INSTANCES / 2]
                );
                keys
            });
        });
    }

    {
        group.bench_function("arrow2/validity_checks", |b| {
            b.iter(|| {
                let data = data.as_any().downcast_ref::<PrimitiveArray<u64>>().unwrap();
                let keys: Vec<InstanceKey> = data
                    .into_iter()
                    .filter_map(|v| v.copied().map(InstanceKey))
                    .collect();
                assert_eq!(NUM_INSTANCES, keys.len());
                assert_eq!(
                    InstanceKey(NUM_INSTANCES as u64 / 2),
                    keys[NUM_INSTANCES / 2]
                );
                keys
            });
        });
    }

    {
        group.bench_function("arrow2/validity_bypass", |b| {
            b.iter(|| {
                let data = data.as_any().downcast_ref::<PrimitiveArray<u64>>().unwrap();
                assert!(data.validity().is_none());
                let keys: Vec<InstanceKey> = data.values_iter().copied().map(InstanceKey).collect();
                assert_eq!(NUM_INSTANCES, keys.len());
                assert_eq!(
                    InstanceKey(NUM_INSTANCES as u64 / 2),
                    keys[NUM_INSTANCES / 2]
                );
                keys
            });
        });
    }
}
