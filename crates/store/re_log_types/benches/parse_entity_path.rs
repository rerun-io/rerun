#![expect(clippy::unwrap_used)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

criterion::criterion_group!(benches, parse_entity_path);
criterion::criterion_main!(benches);

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
                criterion::black_box(entity_path);
            }
        });
    });
}
