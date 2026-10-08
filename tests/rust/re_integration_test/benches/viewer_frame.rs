//! The time of one viewer frame, for a recording of a fleet of robots shown in many views.
//!
//! Every view is a tab in the same container, so only the first one is drawn.
//! The viewer still builds the query results of every view each frame.
//! The selection and time panels are closed.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use re_integration_test::{HarnessExt as _, ViewerHarnessExt as _};
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_chunk::ChunkBuilder;
use re_viewer::external::re_log_types::{EntityPath, EntityPathFilter};
use re_viewer::external::re_sdk_types::archetypes::{
    Boxes2D, Boxes3D, Image, Pinhole, Points3D, Scalars, TextLog, Transform3D,
};
use re_viewer::external::re_sdk_types::components::TextLogLevel;
use re_viewer::external::re_view_spatial::{SpatialView2D, SpatialView3D};
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

// ---

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
mod constants {
    pub const NUM_ROBOTS: &[usize] = &[2];
    pub const NUM_VIEWS: &[usize] = &[1, 4];
}

#[cfg(not(debug_assertions))]
mod constants {
    pub const NUM_ROBOTS: &[usize] = &[20, 100, 400];
    pub const NUM_VIEWS: &[usize] = &[1, 8, 32, 128];
}

use self::constants::{NUM_ROBOTS, NUM_VIEWS};

const NUM_FRAMES: i64 = 10;
const NUM_LIDAR_POINTS: usize = 100;
const NUM_CAMERAS: usize = 4;
const IMAGE_WIDTH: u32 = 32;
const IMAGE_HEIGHT: u32 = 24;
const NUM_ARM_LINKS: usize = 8;
const NUM_JOINTS: usize = 24;

// ---

criterion_group!(benches, viewer_frame_many_views);
criterion_main!(benches);

// ---

fn viewer_frame_many_views(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new() // NOLINT: the benchmark binary owns this runtime
        .expect("Failed to create tokio runtime");
    let _guard = runtime.enter();

    let mut group = c.benchmark_group("viewer_frame");
    group.sample_size(20);

    for &num_robots in NUM_ROBOTS {
        for &num_views in NUM_VIEWS {
            let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
                window_size: Some(egui::vec2(1920.0, 1080.0)),
                ..Default::default()
            });
            harness.init_recording();
            let num_entities = log_fleet(&mut harness, num_robots);
            harness.clear_current_blueprint();

            let tabs = harness.add_blueprint_container(egui_tiles::ContainerKind::Tabs, None);
            let views = fleet_views(num_robots, num_views);
            harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
                blueprint.add_views(views.into_iter(), Some(tabs), None);
            });
            harness.set_selection_panel_opened(false);
            harness.set_time_panel_opened(false);

            // Apply the blueprint changes and build the caches of the first frames, so the
            // benchmark only measures frames where nothing changes.
            harness.run();

            group.bench_function(
                BenchmarkId::from_parameter(format!("{num_entities} entities, {num_views} views")),
                |b| b.iter(|| harness.step()),
            );
        }
    }

    group.finish();
}

// --- Helpers ---

/// Logs a fleet of robots, each with a colored lidar point cloud, a text log, pinhole cameras
/// with an image and labeled detections, an arm of boxes and joint scalars.
/// Returns the number of entities that have data.
fn log_fleet(harness: &mut egui_kittest::Harness<'_, re_viewer::App>, num_robots: usize) -> usize {
    let mut num_entities = 0;

    for robot in 0..num_robots {
        let robot_path = format!("world/robot_{robot}");
        log_frames(harness, &robot_path, |frame| {
            Transform3D::from_translation([robot as f32, frame as f32 * 0.1, 0.0])
        });

        log_frames(harness, &format!("{robot_path}/lidar"), |frame| {
            let positions = (0..NUM_LIDAR_POINTS)
                .map(|i| [i as f32 * 0.1, frame as f32 * 0.1, (i % 10) as f32 * 0.1]);
            Points3D::new(positions)
                .with_colors((0..NUM_LIDAR_POINTS).map(|i| [(i * 2) as u8, 128, 255]))
                .with_radii([0.02])
        });

        log_frames(harness, &format!("{robot_path}/log"), |frame| {
            TextLog::new(format!("robot {robot} reached waypoint {frame}"))
                .with_level(TextLogLevel::INFO)
        });
        num_entities += 3;

        for camera in 0..NUM_CAMERAS {
            let camera_path = format!("{robot_path}/camera_{camera}");
            log_frames(harness, &camera_path, |_| {
                Transform3D::from_translation([0.0, camera as f32, 1.0])
            });
            log_frames(harness, &camera_path, |_| {
                Pinhole::from_focal_length_and_resolution(
                    [IMAGE_WIDTH as f32, IMAGE_WIDTH as f32],
                    [IMAGE_WIDTH as f32, IMAGE_HEIGHT as f32],
                )
            });
            log_frames(harness, &format!("{camera_path}/image"), |frame| {
                let pixels: Vec<u8> = (0..IMAGE_WIDTH * IMAGE_HEIGHT * 3)
                    .map(|i| (i as i64 + frame) as u8)
                    .collect();
                Image::from_rgb24(pixels, [IMAGE_WIDTH, IMAGE_HEIGHT])
            });
            log_frames(harness, &format!("{camera_path}/detections"), |frame| {
                Boxes2D::from_mins_and_sizes(
                    [[frame as f32, 4.0], [20.0, 10.0]],
                    [[8.0, 8.0], [6.0, 12.0]],
                )
                .with_labels(["person", "forklift"])
                .with_class_ids([1, 2])
            });
            num_entities += 3;
        }

        let mut link_path = format!("{robot_path}/arm");
        for link in 0..NUM_ARM_LINKS {
            link_path = format!("{link_path}/link_{link}");
            log_frames(harness, &link_path, |frame| {
                Transform3D::from_translation([0.0, 0.01 * frame as f32, 0.1])
            });
            log_frames(harness, &link_path, |_| {
                Boxes3D::from_half_sizes([[0.02, 0.02, 0.05]]).with_colors([[200, 200, 200]])
            });
            num_entities += 1;
        }

        for joint in 0..NUM_JOINTS {
            log_frames(
                harness,
                &format!("{robot_path}/joints/joint_{joint}"),
                |frame| Scalars::single(joint as f64 + frame as f64),
            );
            num_entities += 1;
        }
    }

    num_entities
}

/// Logs one chunk with a row of the archetype on each frame.
fn log_frames<A: re_sdk::AsComponents>(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    entity_path: &str,
    archetype: impl Fn(i64) -> A,
) {
    let timeline = re_sdk::Timeline::new_sequence("frame");
    harness.log_entity(entity_path, |mut builder: ChunkBuilder| {
        for frame in 0..NUM_FRAMES {
            builder = builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, frame)]),
                &archetype(frame),
            );
        }
        builder
    });
}

/// The first view is a 3D view of everything. The others cycle through a 3D view of one robot,
/// a 2D view of one of its cameras and a time series view of its joints.
fn fleet_views(num_robots: usize, num_views: usize) -> Vec<ViewBlueprint> {
    (0..num_views)
        .map(|i| {
            if i == 0 {
                return ViewBlueprint::new_with_root_wildcard(SpatialView3D::identifier());
            }
            let robot = format!("world/robot_{}", (i / 3) % num_robots);
            let (class, origin, filter) = match i % 3 {
                0 => (
                    SpatialView3D::identifier(),
                    robot,
                    "+ $origin/**\n- $origin/joints/**",
                ),
                1 => (
                    SpatialView2D::identifier(),
                    format!("{robot}/camera_{}", i % NUM_CAMERAS),
                    "+ $origin/**",
                ),
                _ => (
                    re_view_time_series::TimeSeriesView::identifier(),
                    format!("{robot}/joints"),
                    "+ $origin/**",
                ),
            };
            ViewBlueprint::new(
                class,
                RecommendedView {
                    origin: EntityPath::from(origin),
                    query_filter: EntityPathFilter::parse_forgiving(filter),
                },
            )
        })
        .collect()
}
