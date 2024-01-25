//! Example of using the Rerun SDK to log [the Objectron dataset](https://github.com/google-research-datasets/Objectron).
//!
//! Usage:
//! ```sh
//! cargo run -p objectron
//! ```
//! or:
//! ```sh
//! cargo run -p objectron -- --recording chair
//! ```

use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;

use rerun::external::re_log;

// --- Rerun logging ---

struct ArFrame {
    dir: PathBuf,
    data: objectron::ArFrame,
    index: usize,
    timepoint: rerun::TimePoint,
}

impl ArFrame {
    fn from_raw(
        dir: PathBuf,
        index: usize,
        timepoint: rerun::TimePoint,
        ar_frame: objectron::ArFrame,
    ) -> Self {
        Self {
            dir,
            data: ar_frame,
            index,
            timepoint,
        }
    }
}

fn timepoint(index: usize, time: f64) -> rerun::TimePoint {
    let timeline_time = rerun::Timeline::new_temporal("time");
    let timeline_frame = rerun::Timeline::new_sequence("frame");
    let time = rerun::Time::from_seconds_since_epoch(time);
    [
        (timeline_time, time.into()),
        (timeline_frame, (index as i64).into()),
    ]
    .into()
}

struct AnnotationsPerFrame<'a>(HashMap<usize, &'a objectron::FrameAnnotation>);

impl<'a> From<&'a [objectron::FrameAnnotation]> for AnnotationsPerFrame<'a> {
    fn from(anns: &'a [objectron::FrameAnnotation]) -> Self {
        Self(
            anns.iter()
                .map(|ann| (ann.frame_id as usize, ann))
                .collect(),
        )
    }
}

fn log_ar_frame(
    rec: &rerun::RecordingStream,
    annotations: &AnnotationsPerFrame<'_>,
    ar_frame: &ArFrame,
) -> anyhow::Result<()> {
    log_video_frame(rec, ar_frame)?;

    if let Some(ar_camera) = ar_frame.data.camera.as_ref() {
        log_ar_camera(rec, ar_frame.timepoint.clone(), ar_camera)?;
    }

    if let Some(points) = ar_frame.data.raw_feature_points.as_ref() {
        log_feature_points(rec, ar_frame.timepoint.clone(), points)?;
    }

    if let Some(&annotations) = annotations.0.get(&ar_frame.index) {
        log_frame_annotations(rec, &ar_frame.timepoint, annotations)?;
    }

    Ok(())
}

fn log_baseline_objects(
    rec: &rerun::RecordingStream,
    objects: &[objectron::Object],
) -> anyhow::Result<()> {
    let boxes = objects.iter().filter_map(|object| {
        Some({
            if object.r#type != objectron::object::Type::BoundingBox as i32 {
                re_log::warn!(object.r#type, "unsupported type");
                return None;
            }

            let box_half_size: rerun::HalfSizes3D =
                (glam::Vec3::from_slice(&object.scale) * 0.5).into();
            let transform = {
                let translation = glam::Vec3::from_slice(&object.translation);
                // NOTE: the dataset is all row-major, transpose those matrices!
                let rotation = glam::Mat3::from_cols_slice(&object.rotation).transpose();
                rerun::TranslationAndMat3x3::new(translation, rotation)
            };
            let label = object.category.as_str();

            (object.id, box_half_size, transform, label)
        })
    });

    for (id, bbox_half_size, transform, label) in boxes {
        let path = format!("world/annotations/box-{id}");
        rec.log_timeless(
            path.clone(),
            &rerun::Boxes3D::from_half_sizes([bbox_half_size])
                .with_labels([label])
                .with_colors([rerun::Color::from_rgb(160, 230, 130)]),
        )?;
        rec.log_timeless(path, &rerun::Transform3D::new(transform))?;
    }

    Ok(())
}

fn log_video_frame(rec: &rerun::RecordingStream, ar_frame: &ArFrame) -> anyhow::Result<()> {
    let image_path = ar_frame.dir.join(format!("video/{}.jpg", ar_frame.index));
    let img = rerun::datatypes::TensorData::from_jpeg_file(&image_path)?;

    rec.set_timepoint(ar_frame.timepoint.clone());
    rec.log("world/camera", &rerun::Image::new(img))
        .map_err(Into::into)
}

fn log_ar_camera(
    rec: &rerun::RecordingStream,
    timepoint: rerun::TimePoint,
    ar_camera: &objectron::ArCamera,
) -> anyhow::Result<()> {
    // NOTE: the dataset is all row-major, transpose those matrices!
    let world_from_cam = glam::Mat4::from_cols_slice(&ar_camera.transform).transpose();
    let (scale, rot, translation) = world_from_cam.to_scale_rotation_translation();
    assert!((scale - glam::Vec3::ONE).length() < 1e-3);
    let mut intrinsics = glam::Mat3::from_cols_slice(&ar_camera.intrinsics).transpose();
    let w = ar_camera.image_resolution_width.unwrap() as f32;
    let h = ar_camera.image_resolution_height.unwrap() as f32;

    // The actual data is in portrait (1440x1920), but the transforms assume landscape
    // input (1920x1440); we need to convert between the two.
    // See:
    // - https://github.com/google-research-datasets/Objectron/issues/39
    // - https://github.com/google-research-datasets/Objectron/blob/master/notebooks/objectron-3dprojection-hub-tutorial.ipynb
    // swap px/py
    use glam::Vec3Swizzles as _;
    intrinsics.z_axis = intrinsics.z_axis.yxz();
    // swap w/h
    let resolution = glam::Vec2::new(h, w);
    // rotate 90 degrees CCW around 2D plane normal (landscape -> portrait)
    let rot = rot * glam::Quat::from_axis_angle(glam::Vec3::Z, std::f32::consts::TAU / 4.0);
    // TODO(cmc): I can't figure out why I need to do this
    let rot = rot * glam::Quat::from_axis_angle(glam::Vec3::X, std::f32::consts::TAU / 2.0);

    rec.set_timepoint(timepoint);

    rec.log(
        "world/camera",
        &rerun::Transform3D::from_translation_rotation(translation, rot),
    )?;

    rec.log(
        "world/camera",
        &rerun::Pinhole::new(intrinsics)
            // See https://github.com/google-research-datasets/Objectron/issues/39 for coordinate systems
            .with_camera_xyz(rerun::components::ViewCoordinates::RDF)
            .with_resolution(resolution),
    )?;

    Ok(())
}

fn log_feature_points(
    rec: &rerun::RecordingStream,
    timepoint: rerun::TimePoint,
    points: &objectron::ArPointCloud,
) -> anyhow::Result<()> {
    let ids = points.identifier.iter();
    let points = points.point.iter();

    rec.set_timepoint(timepoint);
    rec.log(
        "world/points",
        &rerun::Points3D::new(points.map(|p| {
            (
                p.x.unwrap_or_default(),
                p.y.unwrap_or_default(),
                p.z.unwrap_or_default(),
            )
        }))
        .with_instance_keys(ids.map(|id| rerun::InstanceKey(*id as _)))
        .with_colors([rerun::Color::from_rgb(255, 255, 255)]),
    )?;

    Ok(())
}

fn log_frame_annotations(
    rec: &rerun::RecordingStream,
    timepoint: &rerun::TimePoint,
    annotations: &objectron::FrameAnnotation,
) -> anyhow::Result<()> {
    for ann in &annotations.annotations {
        // TODO(cmc): we shouldn't be using those preprojected 2D points to begin with, Rerun is
        // capable of projecting the actual 3D points in real time now.
        let (ids, points): (Vec<_>, Vec<_>) = ann
            .keypoints
            .iter()
            .filter_map(|kp| {
                kp.point_2d
                    .as_ref()
                    .map(|p| (rerun::InstanceKey(kp.id as _), [p.x * 1440.0, p.y * 1920.0]))
            })
            .unzip();

        rec.set_timepoint(timepoint.clone());

        let ent_path = format!("world/camera/estimates/box-{}", ann.object_id);
        if points.len() == 9 {
            // Build the preprojected bounding box out of 2D line segments.
            #[rustfmt::skip]
            fn linestrips(points: &[[f32; 2]]) -> [[[f32; 2]; 8]; 4] {
                [
                    [
                         points[2], points[1],
                         points[3], points[4],
                         points[4], points[2],
                         points[4], points[3],
                    ],
                    [
                         points[5], points[6],
                         points[5], points[7],
                         points[8], points[6],
                         points[8], points[7],
                    ],
                    [
                         points[1], points[5],
                         points[1], points[3],
                         points[3], points[7],
                         points[5], points[7],
                    ],
                    [
                         points[2], points[6],
                         points[2], points[4],
                         points[4], points[8],
                         points[6], points[8],
                    ],
                ]
            }
            rec.log(
                ent_path,
                &rerun::LineStrips2D::new(linestrips(&points))
                    .with_colors([rerun::Color::from_rgb(130, 160, 250)]),
            )?;
        } else {
            rec.log(
                ent_path,
                &rerun::Points2D::new(points)
                    .with_instance_keys(ids)
                    .with_colors([rerun::Color::from_rgb(130, 160, 250)]),
            )?;
        }
    }

    Ok(())
}

// --- Init ---

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Recording {
    Bike,
    Book,
    Bottle,
    Camera,
    #[value(name("cereal_box"))]
    CerealBox,
    Chair,
    Cup,
    Laptop,
    Shoe,
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Specifies the recording to replay.
    #[clap(long, value_enum, default_value = "book")]
    recording: Recording,

    /// Limits the number of frames logged.
    #[clap(long)]
    frames: Option<usize>,

    /// If set, this indefinitely log and relog the same scene in a loop.
    ///
    /// Useful to test the viewer with a _lot_ of data.
    ///
    /// Needs to be coupled with `--frames` and/or `--connect` for now.
    #[clap(long, default_value = "false")]
    run_forever: bool,

    /// Throttle logging by sleeping between each frame (e.g. `0.25`).
    #[clap(long, value_parser = parse_duration)]
    per_frame_sleep: Option<Duration>,
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseFloatError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs_f64(seconds))
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    // Parse protobuf dataset
    let store_info = args.recording.info().with_context(|| {
        use clap::ValueEnum as _;
        format!(
            "Could not read the recording, have you downloaded the dataset? \
            Try running the python version first to download it automatically \
            (`examples/python/objectron/main.py --recording {}`).",
            args.recording.to_possible_value().unwrap().get_name(),
        )
    })?;
    let annotations = read_annotations(&store_info.path_annotations)?;

    // See https://github.com/google-research-datasets/Objectron/issues/39 for coordinate systems
    rec.log_timeless("world", &rerun::ViewCoordinates::RUB)?;

    log_baseline_objects(rec, &annotations.objects)?;

    let mut global_frame_offset = 0;
    let mut global_time_offset = 0.0;

    'outer: loop {
        let mut frame_offset = 0;
        let mut time_offset = 0.0;

        // Iterate through the parsed dataset and log Rerun primitives
        let ar_frames = read_ar_frames(&store_info.path_ar_frames);
        for (idx, ar_frame) in ar_frames.enumerate() {
            if idx + global_frame_offset >= args.frames.unwrap_or(usize::MAX) {
                break 'outer;
            }

            let ar_frame = ar_frame?;
            let ar_frame = ArFrame::from_raw(
                store_info.path_ar_frames.parent().unwrap().into(),
                idx,
                timepoint(
                    idx + global_frame_offset,
                    ar_frame.timestamp() + global_time_offset,
                ),
                ar_frame,
            );
            let annotations = annotations.frame_annotations.as_slice().into();
            log_ar_frame(rec, &annotations, &ar_frame)?;

            if let Some(d) = args.per_frame_sleep {
                std::thread::sleep(d);
            }

            time_offset = f64::max(time_offset, ar_frame.data.timestamp());
            frame_offset += 1;
        }

        if !args.run_forever {
            break;
        }

        global_time_offset += time_offset;
        global_frame_offset += frame_offset;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_objectron")?;
    run(&rec, &args)
}

// --- Protobuf parsing ---

#[derive(Debug, Clone)]
struct StoreInfo {
    path_ar_frames: PathBuf,
    path_annotations: PathBuf,
}

impl Recording {
    fn info(&self) -> anyhow::Result<StoreInfo> {
        const DATASET_DIR: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../python/objectron/dataset"
        );

        use clap::ValueEnum as _;
        let rec = self.to_possible_value().unwrap();

        // objectron/dataset/book
        let path = PathBuf::from(DATASET_DIR).join(rec.get_name());
        // objectron/dataset/book/batch-20
        let path = std::fs::read_dir(&path)
            .with_context(|| format!("Path: {path:?}"))?
            .next()
            .context("empty directory")??
            .path();
        // objectron/dataset/book/batch-20/35
        let path = std::fs::read_dir(&path)
            .with_context(|| format!("Path: {path:?}"))?
            .next()
            .context("empty directory")??
            .path();

        Ok(StoreInfo {
            // objectron/dataset/book/batch-20/35/geometry.pbdata
            path_ar_frames: path.join("geometry.pbdata"),
            // objectron/dataset/book/batch-20/35/annotation.pbdata
            path_annotations: path.join("annotation.pbdata"),
        })
    }
}

fn read_ar_frames(path: &Path) -> impl Iterator<Item = anyhow::Result<objectron::ArFrame>> + '_ {
    use prost::Message as _;

    re_log::info!(?path, "reading AR frame data");

    let file = std::fs::File::open(path).unwrap();
    let mut reader = std::io::BufReader::with_capacity(1024, file);

    std::iter::from_fn(move || {
        let mut msg_size = [0_u8; 4];
        reader.read_exact(&mut msg_size).ok().map(|_| {
            let msg_size = u32::from_le_bytes(msg_size);
            let mut msg_bytes = vec![0u8; msg_size as _];
            reader.read_exact(&mut msg_bytes)?;

            let ar_frame = objectron::ArFrame::decode(msg_bytes.as_slice())?;
            Ok(ar_frame)
        })
    })
}

fn read_annotations(path: &Path) -> anyhow::Result<objectron::Sequence> {
    use prost::Message as _;
    re_log::info!(?path, "reading annotation data");
    let annotations = objectron::Sequence::decode(std::fs::read(path)?.as_slice())?;
    Ok(annotations)
}

#[rustfmt::skip]
mod objectron;
