#![expect(clippy::cast_possible_wrap)]

use re_byte_size::testing::TrackingAllocator;
use re_byte_size::{BookkeepingBTreeMap, SizeBytes};
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;

#[global_allocator]
pub static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator::system();

#[track_caller]
fn assert_accurate_size<T: SizeBytes>(creator: impl Copy + Fn() -> T) {
    let (value, reported_size) = TrackingAllocator::memory_use(creator);
    assert_eq!(value.heap_size_bytes(), reported_size as u64);

    let (value, reported_size) = TrackingAllocator::memory_use(|| Box::new(creator()));
    assert_eq!(T::total_size_bytes(&*value), reported_size as u64);
}

fn format_result<T: SizeBytes>(type_name: &str, len: usize, creator: impl Fn() -> T) -> String {
    let (value, accurate) = TrackingAllocator::memory_use(creator);
    let estimated = value.heap_size_bytes() as i64;
    let accurate = accurate as i64;

    let error_pct = if accurate == 0 {
        if estimated == 0 { 0.0 } else { f64::INFINITY }
    } else {
        100.0 * (estimated - accurate) as f64 / accurate as f64
    };

    let name = format!("{type_name} len={len}");
    format!(
        "{name:<44} estimated {estimated:>10}B vs {accurate:>10}B actual (error: {error_pct:+.1}%)"
    )
}

fn run_many_sizes<T: SizeBytes>(
    lines: &mut Vec<String>,
    type_name: &str,
    creator: impl Fn(usize) -> T,
) {
    for len in [0usize, 1, 2, 3, 10, 100, 1_000, 10_000] {
        lines.push(format_result(type_name, len, || creator(len)));
    }
}

#[test]
fn test_sizes() {
    // These should be exact
    assert_accurate_size(|| [0u8; 16]);
    assert_accurate_size(|| String::from("Hello there!"));
    assert_accurate_size(|| Box::new(String::from("Hello there!")));
    assert_accurate_size(|| Arc::new(String::from("Hello there!")));
    assert_accurate_size(|| vec![0u8; 1024 * 1024]);
    assert_accurate_size(|| Vec::<String>::with_capacity(100));

    // These are estimates - collect them all into a single snapshot
    let mut lines: Vec<String> = Vec::new();

    run_many_sizes(&mut lines, "BTreeSet<String>", |len| {
        (0..len)
            .map(|i| i.to_string())
            .collect::<BTreeSet<String>>()
    });
    run_many_sizes(&mut lines, "BTreeSet<u128>", |len| {
        (0..len as u128).collect::<BTreeSet<u128>>()
    });
    run_many_sizes(&mut lines, "BTreeSet<u32>", |len| {
        (0..len as u32).collect::<BTreeSet<u32>>()
    });

    lines.push(String::new());

    run_many_sizes(&mut lines, "BTreeMap<u128, [u8; 256]>", |len| {
        (0..len as u128)
            .map(|i| (i, [i as u8; 256]))
            .collect::<BTreeMap<u128, [u8; 256]>>()
    });
    run_many_sizes(&mut lines, "BTreeMap<u32, u8>", |len| {
        (0..len as u32)
            .map(|i| (i, i as u8))
            .collect::<BTreeMap<u32, u8>>()
    });
    run_many_sizes(&mut lines, "BTreeMap<u64, String> (collect)", |len| {
        (0..len as u64)
            .map(|i| (i, format!("value_{i}")))
            .collect::<BTreeMap<u64, String>>()
    });
    run_many_sizes(&mut lines, "BTreeMap<u64, String> (insert)", |len| {
        // Pre-create all strings first to isolate BTree allocation behavior
        let items: Vec<_> = (0..len as u64).map(|i| (i, format!("value_{i}"))).collect();
        let mut map = BTreeMap::<u64, String>::new();
        for (k, v) in items {
            map.insert(k, v);
        }
        map
    });

    lines.push(String::new());

    run_many_sizes(&mut lines, "BookkeepingBTreeMap<u64, String>", |len| {
        let mut map = BookkeepingBTreeMap::<u64, String>::new();
        for i in 0..len as u64 {
            map.insert(i, format!("value_{i}"));
        }
        map
    });

    lines.push(String::new());

    run_many_sizes(&mut lines, "HashSet<String>", |len| {
        (0..len).map(|i| i.to_string()).collect::<HashSet<String>>()
    });
    run_many_sizes(&mut lines, "HashSet<u128>", |len| {
        (0..len as u128).collect::<HashSet<u128>>()
    });
    run_many_sizes(&mut lines, "HashSet<u32>", |len| {
        (0..len as u32).collect::<HashSet<u32>>()
    });

    lines.push(String::new());

    run_many_sizes(&mut lines, "HashMap<u128, [u8; 256]>", |len| {
        (0..len as u128)
            .map(|i| (i, [i as u8; 256]))
            .collect::<HashMap<u128, [u8; 256]>>()
    });
    run_many_sizes(&mut lines, "HashMap<u32, u8>", |len| {
        (0..len as u32)
            .map(|i| (i, i as u8))
            .collect::<HashMap<u32, u8>>()
    });

    lines.push(String::new());

    // SmallVec with capacity 4 - test both inline and spilled cases
    run_many_sizes(&mut lines, "SmallVec<[String; 4]>", |len| {
        (0..len)
            .map(|i| i.to_string())
            .collect::<SmallVec<[String; 4]>>()
    });
    run_many_sizes(&mut lines, "SmallVec<[u32; 4]>", |len| {
        (0..len as u32).collect::<SmallVec<[u32; 4]>>()
    });

    run_many_sizes(&mut lines, "VecDeque<String>", |len| {
        (0..len)
            .map(|i| i.to_string())
            .collect::<VecDeque<String>>()
    });
    run_many_sizes(&mut lines, "VecDeque<u32>", |len| {
        (0..len as u32).collect::<VecDeque<u32>>()
    });

    insta::assert_snapshot!(lines.join("\n"));
}
