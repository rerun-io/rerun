#![expect(clippy::unwrap_used)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion::criterion_group!(benches, parse_entity_path, common_ancestor);
criterion::criterion_main!(benches);

const NUM_ENTITIES: usize = 32;

fn parse_entity_path(c: &mut criterion::Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let paths = [
        "root",
        "/root/child",
        "root/child/grandchild",
        "/root/child/grandchild/great_grandchild",
        "root/child/grandchild/great_grandchild/great_great_grandchild",
        "/a/very/long/entity/path/with/many/segments/to/test/the/parsing/performance/in/the/benchmarks",
    ];

    let num = 10_000;

    let mut group = c.benchmark_group("EntityPath");
    group.throughput(criterion::Throughput::Elements(num as _));

    group.bench_function("parse_entity_path", |b| {
        let mut strings_iter = paths.iter().cycle();

        b.iter(|| {
            for _ in 0..num {
                let path_str = strings_iter.next().unwrap();
                let entity_path = re_log_types::EntityPath::parse_forgiving(path_str);
                std::hint::black_box(entity_path);
            }
        });
    });
}

fn common_ancestor(c: &mut criterion::Criterion) {
    if std::env::var("CI").is_ok() {
        return;
    }

    let entities = (0..NUM_ENTITIES)
        .map(|entity_idx| {
            re_log_types::EntityPath::from(format!("world/site/cameras/camera_{entity_idx}/sensor"))
        })
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("EntityPath/common_ancestor/32");
    group.throughput(criterion::Throughput::Elements(NUM_ENTITIES as _));
    group.bench_function("compute", |b| {
        b.iter(|| {
            re_log_types::EntityPath::common_ancestor_of(std::hint::black_box(entities.iter()))
        });
    });
}
