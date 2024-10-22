use criterion::{criterion_group, criterion_main, Criterion};

fn video_load(c: &mut Criterion) {
    let video = include_bytes!("../../../../tests/assets/video/Big_Buck_Bunny_1080_10s_av1.mp4");
    c.bench_function("video_load", |b| {
        b.iter_batched(
            || {},
            |()| re_video::VideoData::load_from_bytes(video, "video/mp4"),
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, video_load);
criterion_main!(benches);
