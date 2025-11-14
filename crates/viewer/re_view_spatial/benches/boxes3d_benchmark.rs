use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use re_log_types::TimePoint;
use re_renderer::Color32;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{RowId, archetypes, components::FillMode};
use re_view_spatial::SpatialView3D;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

/// Benchmark rendering various numbers of Boxes3D primitives.
/// This measures the end-to-end performance of the Boxes3D visualizer,
/// including fast path vs slow path selection and GPU upload.
fn boxes3d_rendering_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("boxes3d_rendering");

    // Test various box counts to see where the fast path threshold matters
    let box_counts = [
        100,    // Well below threshold (1000)
        500,    // Below threshold
        1_000,  // At threshold
        5_000,  // Above threshold - should use fast path
        10_000, // Well above threshold
        50_000, // Large scale
    ];

    for &num_boxes in &box_counts {
        group.throughput(Throughput::Elements(num_boxes as u64));

        // Benchmark: Translation-only boxes (fast path)
        group.bench_with_input(
            BenchmarkId::new("translation_only", num_boxes),
            &num_boxes,
            |b, &count| {
                // Set up the test context and data once outside the benchmark loop
                let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

                // Create boxes in a grid
                let grid_size = (count as f32).cbrt().ceil() as usize;
                let mut centers = Vec::with_capacity(count);
                let mut half_sizes = Vec::with_capacity(count);
                let mut colors = Vec::with_capacity(count);

                for i in 0..grid_size {
                    for j in 0..grid_size {
                        for k in 0..grid_size {
                            if centers.len() >= count {
                                break;
                            }

                            let x = i as f32 * 2.0;
                            let y = j as f32 * 2.0;
                            let z = k as f32 * 2.0;

                            centers.push([x, y, z]);
                            half_sizes.push([0.4, 0.4, 0.4]);
                            colors.push(Color32::from_rgba_unmultiplied(100, 150, 200, 255));
                        }
                    }
                }

                test_context.log_entity("boxes", |builder| {
                    builder.with_archetype(
                        RowId::new(),
                        TimePoint::default(),
                        &archetypes::Boxes3D::from_half_sizes(half_sizes)
                            .with_centers(centers)
                            .with_colors(colors)
                            .with_fill_mode(FillMode::Solid),
                    )
                });

                // Set up viewport and view
                let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
                    let view_blueprint = ViewBlueprint::new(
                        SpatialView3D::identifier(),
                        RecommendedView {
                            origin: "/boxes".into(),
                            query_filter: "+ $origin/**".parse().unwrap(),
                        },
                    );
                    let view_id = view_blueprint.id;
                    blueprint.add_views(std::iter::once(view_blueprint), None, None);
                    view_id
                });

                let mut harness = test_context
                    .setup_kittest_for_rendering_3d(egui::Vec2::new(800.0, 600.0))
                    .build_ui(|ui| {
                        test_context.run_with_single_view(ui, view_id);
                    });

                // Benchmark the actual rendering
                b.iter(|| {
                    // Run one frame which executes the visualizer system
                    harness.run_steps(1);
                    black_box(&harness);
                });
            },
        );

        // Benchmark: Boxes with rotations (slow path)
        // Only test this for smaller counts as it's intentionally slow
        if num_boxes <= 5_000 {
            group.bench_with_input(
                BenchmarkId::new("with_rotations", num_boxes),
                &num_boxes,
                |b, &count| {
                    // Set up the test context and data once outside the benchmark loop
                    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

                    let grid_size = (count as f32).cbrt().ceil() as usize;
                    let mut centers = Vec::with_capacity(count);
                    let mut half_sizes = Vec::with_capacity(count);
                    let mut rotations = Vec::with_capacity(count);
                    let mut colors = Vec::with_capacity(count);

                    for i in 0..grid_size {
                        for j in 0..grid_size {
                            for k in 0..grid_size {
                                if centers.len() >= count {
                                    break;
                                }

                                let x = i as f32 * 2.0;
                                let y = j as f32 * 2.0;
                                let z = k as f32 * 2.0;

                                // Simple rotation for benchmarking
                                let angle = (i + j + k) as f32 * 0.1;
                                let half_angle = angle / 2.0;
                                let (sin, cos) = half_angle.sin_cos();

                                centers.push([x, y, z]);
                                half_sizes.push([0.8, 0.3, 0.3]);
                                // Z-axis rotation quaternion
                                rotations.push(re_types::datatypes::Quaternion([
                                    0.0,
                                    0.0,
                                    sin,
                                    cos,
                                ]));
                                colors.push(Color32::from_rgba_unmultiplied(200, 100, 150, 255));
                            }
                        }
                    }

                    test_context.log_entity("boxes", |builder| {
                        builder.with_archetype(
                            RowId::new(),
                            TimePoint::default(),
                            &archetypes::Boxes3D::from_half_sizes(half_sizes)
                                .with_centers(centers)
                                .with_quaternions(rotations)
                                .with_colors(colors)
                                .with_fill_mode(FillMode::Solid),
                        )
                    });

                    // Set up viewport and view
                    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
                        let view_blueprint = ViewBlueprint::new(
                            SpatialView3D::identifier(),
                            RecommendedView {
                                origin: "/boxes".into(),
                                query_filter: "+ $origin/**".parse().unwrap(),
                            },
                        );
                        let view_id = view_blueprint.id;
                        blueprint.add_views(std::iter::once(view_blueprint), None, None);
                        view_id
                    });

                    let mut harness = test_context
                        .setup_kittest_for_rendering_3d(egui::Vec2::new(800.0, 600.0))
                        .build_ui(|ui| {
                            test_context.run_with_single_view(ui, view_id);
                        });

                    // Benchmark the actual rendering
                    b.iter(|| {
                        // Run one frame which executes the visualizer system
                        harness.run_steps(1);
                        black_box(&harness);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark the fast path threshold decision logic.
/// This measures just the overhead of determining whether to use fast vs slow path.
fn boxes3d_path_selection_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("boxes3d_path_selection");

    // Simulate the transform checking logic
    let transforms: Vec<glam::DAffine3> = (0..10_000)
        .map(|i| {
            let x = (i / 100) as f64;
            let y = (i % 100) as f64;
            glam::DAffine3::from_translation(glam::DVec3::new(x, y, 0.0))
        })
        .collect();

    group.bench_function("is_transform_trivial", |b| {
        b.iter(|| {
            let is_trivial = transforms.iter().all(|t| {
                // Inline the transform checking logic
                let matrix3 = t.matrix3;
                let identity = glam::DMat3::IDENTITY;
                const EPSILON: f64 = 1e-6;

                let mut trivial = true;
                for i in 0..3 {
                    for j in 0..3 {
                        if (matrix3.col(j)[i] - identity.col(j)[i]).abs() > EPSILON {
                            trivial = false;
                            break;
                        }
                    }
                    if !trivial {
                        break;
                    }
                }
                trivial
            });

            black_box(is_trivial);
        });
    });

    group.finish();
}

criterion_group!(benches, boxes3d_rendering_benchmark, boxes3d_path_selection_benchmark);
criterion_main!(benches);
