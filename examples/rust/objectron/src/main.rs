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

use anyhow::{anyhow, Context as _};

use rerun::{
    external::re_log,
    time::{Time, TimePoint, TimeType, Timeline},
    MsgSender, RecordingStream,
};

// --- Rerun logging ---

struct ArFrame {
    dir: PathBuf,
    data: objectron::ArFrame,
    index: usize,
    timepoint: TimePoint,
}

impl ArFrame {
    fn from_raw(
        dir: PathBuf,
        index: usize,
        timepoint: TimePoint,
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

fn timepoint(index: usize, time: f64) -> TimePoint {
    let timeline_time = Timeline::new("time", TimeType::Time);
    let timeline_frame = Timeline::new("frame", TimeType::Sequence);
    let time = Time::from_seconds_since_epoch(time);
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

fn log_coordinate_space(
    rec_stream: &RecordingStream,
    ent_path: impl Into<rerun::EntityPath>,
    axes: &str,
) -> anyhow::Result<()> {
    let view_coords: rerun::components::ViewCoordinates = axes
        .parse()
        .map_err(|err| anyhow!("couldn't parse {axes:?} as ViewCoordinates: {err}"))?;
    MsgSender::new(ent_path)
        .with_timeless(true)
        .with_component(&[view_coords])?
        .send(rec_stream)
        .map_err(Into::into)
}

fn log_ar_frame(
    rec_stream: &RecordingStream,
    annotations: &AnnotationsPerFrame<'_>,
    ar_frame: &ArFrame,
) -> anyhow::Result<()> {
    log_video_frame(rec_stream, ar_frame)?;

    if let Some(ar_camera) = ar_frame.data.camera.as_ref() {
        log_ar_camera(rec_stream, ar_frame.timepoint.clone(), ar_camera)?;
    }

    if let Some(points) = ar_frame.data.raw_feature_points.as_ref() {
        log_feature_points(rec_stream, ar_frame.timepoint.clone(), points)?;
    }

    if let Some(&annotations) = annotations.0.get(&ar_frame.index) {
        log_frame_annotations(rec_stream, &ar_frame.timepoint, annotations)?;
    }

    Ok(())
}

fn log_baseline_objects(
    rec_stream: &RecordingStream,
    objects: &[objectron::Object],
) -> anyhow::Result<()> {
    use rerun::components::{Box3D, ColorRGBA, Label, Transform3D};
    use rerun::transform::TranslationAndMat3;

    let boxes = objects.iter().filter_map(|object| {
        Some({
            if object.r#type != objectron::object::Type::BoundingBox as i32 {
                re_log::warn!(object.r#type, "unsupported type");
                return None;
            }

            let box3: Box3D = glam::Vec3::from_slice(&object.scale).into();
            let transform = {
                let translation = glam::Vec3::from_slice(&object.translation);
                // NOTE: the dataset is all row-major, transpose those matrices!
                let rotation = glam::Mat3::from_cols_slice(&object.rotation).transpose();
                Transform3D::new(TranslationAndMat3::new(translation, rotation))
            };
            let label = Label(object.category.clone());

            (object.id, box3, transform, label)
        })
    });

    for (id, bbox, transform, label) in boxes {
        MsgSender::new(format!("world/annotations/box-{id}"))
            .with_timeless(true)
            .with_component(&[bbox])?
            .with_component(&[transform])?
            .with_component(&[label])?
            .with_splat(ColorRGBA::from_rgb(160, 230, 130))?
            .send(rec_stream)?;
    }

    Ok(())
}

fn log_video_frame(rec_stream: &RecordingStream, ar_frame: &ArFrame) -> anyhow::Result<()> {
    let image_path = ar_frame.dir.join(format!("video/{}.jpg", ar_frame.index));
    let tensor = rerun::components::Tensor::from_jpeg_file(&image_path)?;

    MsgSender::new("world/camera/video")
        .with_timepoint(ar_frame.timepoint.clone())
        .with_component(&[tensor])?
        .send(rec_stream)?;

    Ok(())
}

fn log_ar_camera(
    rec_stream: &RecordingStream,
    timepoint: TimePoint,
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

    use rerun::components::{Pinhole, Transform3D};
    use rerun::transform::TranslationRotationScale3D;
    MsgSender::new("world/camera")
        .with_timepoint(timepoint.clone())
        .with_component(&[Transform3D::new(TranslationRotationScale3D::rigid(
            translation,
            rot,
        ))])?
        .send(rec_stream)?;
    MsgSender::new("world/camera/video")
        .with_timepoint(timepoint)
        .with_component(&[Pinhole {
            image_from_cam: intrinsics.into(),
            resolution: Some(resolution.into()),
        }])?
        .send(rec_stream)?;

    Ok(())
}

fn log_feature_points(
    rec_stream: &RecordingStream,
    timepoint: TimePoint,
    points: &objectron::ArPointCloud,
) -> anyhow::Result<()> {
    use rerun::components::{ColorRGBA, InstanceKey, Point3D};

    let ids = points.identifier.iter();
    let points = points.point.iter();
    let (ids, points): (Vec<_>, Vec<_>) = ids
        .zip(points)
        .map(|(id, p)| {
            (
                InstanceKey(*id as _),
                Point3D::new(
                    p.x.unwrap_or_default(),
                    p.y.unwrap_or_default(),
                    p.z.unwrap_or_default(),
                ),
            )
        })
        .unzip();

    MsgSender::new("world/points")
        .with_timepoint(timepoint)
        .with_component(&points)?
        .with_component(&ids)?
        .with_splat(ColorRGBA::from_rgb(255, 255, 255))?
        .send(rec_stream)?;

    Ok(())
}

fn log_frame_annotations(
    rec_stream: &RecordingStream,
    timepoint: &TimePoint,
    annotations: &objectron::FrameAnnotation,
) -> anyhow::Result<()> {
    use rerun::components::{ColorRGBA, InstanceKey, LineStrip2D, Point2D};

    for ann in &annotations.annotations {
        // TODO(cmc): we shouldn't be using those preprojected 2D points to begin with, Rerun is
        // capable of projecting the actual 3D points in real time now.
        let (ids, points): (Vec<_>, Vec<_>) = ann
            .keypoints
            .iter()
            .filter_map(|kp| {
                kp.point_2d
                    .as_ref()
                    .map(|p| (InstanceKey(kp.id as _), [p.x * 1440.0, p.y * 1920.0]))
            })
            .unzip();

        let mut msg = MsgSender::new(format!(
            "world/camera/video/estimates/box-{}",
            ann.object_id
        ))
        .with_timepoint(timepoint.clone())
        .with_splat(ColorRGBA::from_rgb(130, 160, 250))?;

        if points.len() == 9 {
            // Build the preprojected bounding box out of 2D line segments.
            #[rustfmt::skip]
            fn linestrips(points: &[[f32; 2]]) -> Vec<LineStrip2D> {
                vec![
                    vec![
                         points[2], points[1],
                         points[3], points[4],
                         points[4], points[2],
                         points[4], points[3],
                    ].into(),
                    vec![
                         points[5], points[6],
                         points[5], points[7],
                         points[8], points[6],
                         points[8], points[7],
                    ].into(),
                    vec![
                         points[1], points[5],
                         points[1], points[3],
                         points[3], points[7],
                         points[5], points[7],
                    ].into(),
                    vec![
                         points[2], points[6],
                         points[2], points[4],
                         points[4], points[8],
                         points[6], points[8],
                    ].into(),
                ]
            }
            msg = msg.with_component(&linestrips(&points))?;
        } else {
            msg = msg
                .with_component(&ids)?
                .with_component(&points.into_iter().map(Point2D::from).collect::<Vec<_>>())?;
        }

        msg.send(rec_stream)?;
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

fn run(rec_stream: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    // Parse protobuf dataset
    let rec_info = args.recording.info().with_context(|| {
        use clap::ValueEnum as _;
        format!(
            "Could not read the recording, have you downloaded the dataset? \
            Try running the python version first to download it automatically \
            (`examples/python/objectron/main.py --recording {}`).",
            args.recording.to_possible_value().unwrap().get_name(),
        )
    })?;
    let annotations = read_annotations(&rec_info.path_annotations)?;

    // See https://github.com/google-research-datasets/Objectron/issues/39
    log_coordinate_space(rec_stream, "world", "RUB")?;
    log_coordinate_space(rec_stream, "world/camera", "RDF")?;

    log_baseline_objects(rec_stream, &annotations.objects)?;

    let mut global_frame_offset = 0;
    let mut global_time_offset = 0.0;

    'outer: loop {
        let mut frame_offset = 0;
        let mut time_offset = 0.0;

        // Iterate through the parsed dataset and log Rerun primitives
        let ar_frames = read_ar_frames(&rec_info.path_ar_frames);
        for (idx, ar_frame) in ar_frames.enumerate() {
            if idx + global_frame_offset >= args.frames.unwrap_or(usize::MAX) {
                break 'outer;
            }

            let ar_frame = ar_frame?;
            let ar_frame = ArFrame::from_raw(
                rec_info.path_ar_frames.parent().unwrap().into(),
                idx,
                timepoint(
                    idx + global_frame_offset,
                    ar_frame.timestamp() + global_time_offset,
                ),
                ar_frame,
            );
            let annotations = annotations.frame_annotations.as_slice().into();
            log_ar_frame(rec_stream, &annotations, &ar_frame)?;

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
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun
        .clone()
        .run("objectron_rs", default_enabled, move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        })
}

// --- Protobuf parsing ---

#[derive(Debug, Clone)]
struct RecordingInfo {
    path_ar_frames: PathBuf,
    path_annotations: PathBuf,
}

impl Recording {
    fn info(&self) -> anyhow::Result<RecordingInfo> {
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

        Ok(RecordingInfo {
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
