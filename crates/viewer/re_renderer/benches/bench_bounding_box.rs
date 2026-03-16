use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec3;

fn generate_point_cloud(n: usize, num_outliers: usize) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(n + num_outliers);

    // Cluster of points around (5, 10, 15) with spread ~2.
    for i in 0..n {
        let t = i as f32 / n as f32;
        points.push(Vec3::new(
            5.0 + (t * 17.3).sin() * 2.0,
            10.0 + (t * 31.7).cos() * 2.0,
            15.0 + (t * 53.1).sin() * 2.0,
        ));
    }

    // Outliers far away.
    for i in 0..num_outliers {
        let v = (i + 1) as f32 * 1000.0;
        points.push(Vec3::new(v, -v, v));
    }

    points
}

fn bench_bounding_boxes(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounding_box");

    for n in [100, 1_000, 10_000, 100_000] {
        let points = generate_point_cloud(n, n / 100);

        group.throughput(criterion::Throughput::Elements(points.len() as u64));
        group.bench_function(format!("naive/{n}"), |b| {
            b.iter(|| {
                criterion::black_box(re_renderer::util::bounding_box_from_points(
                    points.iter().copied(),
                ))
            });
        });
        group.bench_function(format!("point_cloud_bounds/{n}"), |b| {
            b.iter(|| criterion::black_box(re_renderer::util::point_cloud_bounds(&points)));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_bounding_boxes);
criterion_main!(benches);
