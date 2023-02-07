//! Example of using the Rerun SDK to log the Objectron dataset.
//!
//! Usage:
//! ```
//! cargo run -p objectron -- --recording chair
//! ```

use std::{
    collections::HashMap,
    fmt::format,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context};
use clap::Parser;
use glam::Vec3Swizzles;
use image::ImageDecoder;
use prost::Message;
use rerun::{
    external::{re_log, re_log_types::ApplicationId, re_memory::AccountingAllocator, re_sdk_comms},
    Box3D, ColorRGBA, EntityPath, InstanceKey, Label, LineStrip2D, LineStrip3D, Mesh3D, MeshId,
    MsgSender, Point2D, Point3D, Quaternion, RawMesh3D, RecordingId, Rigid3, Session, Tensor,
    TensorData, TensorDataMeaning, TensorDimension, TensorId, Time, TimePoint, TimeType, Timeline,
    Transform, Vec3D, Vec4D, ViewCoordinates,
};

// --- Rerun logging ---

struct ArFrame {
    dir: PathBuf,
    data: objectron::ArFrame,
    index: usize,
    timepoint: TimePoint,
}
impl ArFrame {
    fn from_raw(dir: PathBuf, index: usize, ar_frame: objectron::ArFrame) -> Self {
        let timeline_time = Timeline::new("time", TimeType::Time);
        let timeline_frame = Timeline::new("frame", TimeType::Sequence);
        let time = Time::from_seconds_since_epoch(ar_frame.timestamp());
        Self {
            dir,
            data: ar_frame,
            index,
            timepoint: [
                (timeline_time, time.into()),
                (timeline_frame, (index as i64).into()),
            ]
            .into(),
        }
    }
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
    session: &mut Session,
    ent_path: impl Into<EntityPath>,
    axes: &str,
) -> anyhow::Result<()> {
    let view_coords: ViewCoordinates = axes
        .parse()
        .map_err(|err| anyhow!("couldn't parse {axes:?} as ViewCoordinates: {err}"))?;
    MsgSender::new(ent_path)
        .with_timeless(true)
        .with_component(&[view_coords])?
        .send(session)
        .map_err(Into::into)
}

fn log_ar_frame(
    session: &mut Session,
    objects: &[objectron::Object],
    annotations: &AnnotationsPerFrame<'_>,
    ar_frame: &ArFrame,
) -> anyhow::Result<()> {
    log_detected_objects(session, objects)?;

    log_video_frame(session, ar_frame)?;

    if let Some(ar_camera) = ar_frame.data.camera.as_ref() {
        log_ar_camera(session, ar_frame.timepoint.clone(), ar_camera)?;
    }

    if let Some(points) = ar_frame.data.raw_feature_points.as_ref() {
        log_feature_points(session, ar_frame.timepoint.clone(), points)?;
    }

    if let Some(&annotations) = annotations.0.get(&ar_frame.index) {
        log_annotations(session, &ar_frame.timepoint, annotations)?;
    }

    Ok(())
}

// TODO(cmc): This whole thing with static objects should probably just be dropped?
fn log_detected_objects(
    session: &mut Session,
    objects: &[objectron::Object],
) -> anyhow::Result<()> {
    let (boxes, transforms, labels): (Vec<_>, Vec<_>, Vec<_>) =
        itertools::multiunzip(objects.iter().filter_map(|object| {
            Some({
                if object.r#type != objectron::object::Type::BoundingBox as i32 {
                    re_log::warn!(object.r#type, "unsupported type");
                    return None;
                }

                let box3: Box3D = glam::Vec3::from_slice(&object.scale).into();
                let transform = {
                    let translation = glam::Vec3::from_slice(&object.translation).into();
                    // NODE: the dataset is all row-major, transpose those matrices!
                    let rotation = glam::Mat3::from_cols_slice(&object.rotation).transpose();
                    let rotation = glam::Quat::from_mat3(&rotation).into();
                    Transform::Rigid3(Rigid3 {
                        rotation,
                        translation,
                    })
                };
                let label = Label(object.category.clone());

                (box3, transform, label)
            })
        }));

    MsgSender::new("world/objects/boxes")
        .with_timeless(true)
        .with_component(&boxes)?
        .with_component(&transforms)?
        .with_component(&labels)?
        .with_splat(ColorRGBA::from([130, 160, 250, 255]))?
        .send(session)?;

    Ok(())
}

fn log_video_frame(session: &mut Session, ar_frame: &ArFrame) -> anyhow::Result<()> {
    let image_path = ar_frame.dir.join(format!("video/{}.jpg", ar_frame.index));
    let jpeg_bytes = std::fs::read(image_path)?;
    let jpeg = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&jpeg_bytes))?;
    assert_eq!(jpeg.color_type(), image::ColorType::Rgb8); // TODO(emilk): support gray-scale jpeg aswell
    let (w, h) = jpeg.dimensions();

    MsgSender::new("world/camera/video")
        .with_timepoint(ar_frame.timepoint.clone())
        // TODO(cmc): `Tensor` should have an `image` integration really
        .with_component(&[Tensor {
            tensor_id: TensorId::random(),
            shape: vec![
                TensorDimension::height(h as _),
                TensorDimension::width(w as _),
                TensorDimension::depth(3),
            ],
            data: TensorData::JPEG(jpeg_bytes),
            meaning: TensorDataMeaning::Unknown,
            meter: None,
        }])?
        .send(session)?;

    Ok(())
}

fn log_ar_camera(
    session: &mut Session,
    timepoint: TimePoint,
    ar_camera: &objectron::ArCamera,
) -> anyhow::Result<()> {
    // NODE: the dataset is all row-major, transpose those matrices!
    let world_from_cam = glam::Mat4::from_cols_slice(&ar_camera.transform).transpose();
    let (scale, rot, translation) = world_from_cam.to_scale_rotation_translation();
    assert!((scale - glam::Vec3::ONE).length() < 1e-3);
    let mut intrinsics = glam::Mat3::from_cols_slice(&ar_camera.intrinsics).transpose();
    let w = ar_camera.image_resolution_width.unwrap() as f32;
    let h = ar_camera.image_resolution_height.unwrap() as f32;

    // The actual data is in portait (1440x1920), but the transforms assume landscape
    // input (1920x1440); we need to convert between the two.
    // See:
    // - https://github.com/google-research-datasets/Objectron/issues/39
    // - https://github.com/google-research-datasets/Objectron/blob/master/notebooks/objectron-3dprojection-hub-tutorial.ipynb
    // swap px/py
    intrinsics.z_axis = intrinsics.z_axis.yxz();
    // swap w/h
    let resolution = glam::Vec2::new(h, w);
    // rotate 90 degrees CCW around 2D plane normal (landscape -> portait)
    let rot = rot * glam::Quat::from_axis_angle(glam::Vec3::Z, std::f32::consts::TAU / 4.0);
    // TODO(cmc): I can't figure out why I need to do this
    let rot = rot * glam::Quat::from_axis_angle(glam::Vec3::X, std::f32::consts::TAU / 2.0);

    MsgSender::new("world/camera")
        .with_timepoint(timepoint.clone())
        .with_component(&[Transform::Rigid3(Rigid3 {
            rotation: rot.into(),
            translation: translation.into(),
        })])?
        .send(session)?;

    MsgSender::new("world/camera/video")
        .with_timepoint(timepoint)
        .with_component(&[Transform::Pinhole(rerun::Pinhole {
            image_from_cam: intrinsics.into(),
            resolution: Some(resolution.into()),
        })])?
        .send(session)?;

    Ok(())
}

fn log_feature_points(
    session: &mut Session,
    timepoint: TimePoint,
    points: &objectron::ArPointCloud,
) -> anyhow::Result<()> {
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
        .with_splat(ColorRGBA::from([255, 255, 255, 255]))?
        .send(session)?;

    Ok(())
}

fn log_annotations(
    session: &mut Session,
    timepoint: &TimePoint,
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
                    .map(|p| (InstanceKey(kp.id as _), [p.x * 1440.0, p.y * 1920.0]))
            })
            .unzip();

        let mut msg = MsgSender::new(format!(
            "world/camera/video/obj-annotations/annotation-{}",
            ann.object_id
        ))
        .with_timepoint(timepoint.clone())
        .with_splat(ColorRGBA::from([130, 160, 250, 255]))?;

        if points.len() == 9 {
            // Build the preprojected bounding box out of 2D line segments.
            #[rustfmt::skip]
            fn linestrips(points: &[[f32; 2]]) -> Vec<LineStrip2D> {
                vec![
                    LineStrip2D::from(vec![
                         points[2], points[1],
                         points[3], points[4],
                         points[4], points[2],
                         points[4], points[3],
                    ]),
                    LineStrip2D::from(vec![
                         points[5], points[6],
                         points[5], points[7],
                         points[8], points[6],
                         points[8], points[7],
                    ]),
                    LineStrip2D::from(vec![
                         points[1], points[5],
                         points[1], points[3],
                         points[3], points[7],
                         points[5], points[7],
                    ]),
                    LineStrip2D::from(vec![
                         points[2], points[6],
                         points[2], points[4],
                         points[4], points[8],
                         points[6], points[8],
                    ]),
                ]
            }
            msg = msg.with_component(&linestrips(&points))?;
        } else {
            msg = msg
                .with_component(&ids)?
                .with_component(&points.into_iter().map(Point2D::from).collect::<Vec<_>>())?;
        }

        msg.send(session)?;
    }

    Ok(())
}

// --- Init ---

// Use MiMalloc as global allocator (because it is fast), wrapped in Rerun's allocation tracker
// so that the rerun viewer can show how much memory it is using when calling `show`.
#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

// TODO: propose to run the python version first if not found
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

// TODO:
// --frames FRAMES       If specified, limits the number of frames logged
// --run-forever         Run forever, continually logging data.
// --per-frame-sleep PER_FRAME_SLEEP
//                       Sleep this much for each frame read, if --run-forever
#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// If specified, connects and sends the logged data to a remote Rerun viewer.
    #[clap(long)]
    #[allow(clippy::option_option)]
    connect: Option<Option<String>>,

    /// Specifies the recording to replay.
    #[clap(long, value_enum, default_value = "book")]
    recording: Recording,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    let args = Args::parse();
    let addr = match args.connect.as_ref() {
        Some(Some(addr)) => Some(addr.parse()?),
        Some(None) => Some(rerun::default_server_addr()),
        None => None,
    };

    let mut session = rerun::Session::new();
    // TODO(cmc): application id should take selected recording into account
    // TODO(cmc): The Rust SDK needs a higher-level `init()` method, akin to what the python SDK
    // does... which they can probably share.
    // This needs to take care of the whole `official_example` thing, and also keeps track of
    // whether we're using the rust or python sdk.
    session.set_application_id(ApplicationId("api_demo_rs".to_owned()), true);
    session.set_recording_id(RecordingId::random());
    if let Some(addr) = addr {
        session.connect(addr);
    }

    // Parse protobuf dataset
    let rec_info = args.recording.info()?;
    let annotations = read_annotations(&rec_info.path_annotations)?;
    let ar_frames = read_ar_frames(&rec_info.path_ar_frames);

    // See https://github.com/google-research-datasets/Objectron/issues/39
    log_coordinate_space(&mut session, "world", "RUB")?;
    log_coordinate_space(&mut session, "world/camera", "RDF")?;

    // TODO: run-forever
    // let mut time_offset = 0;
    // let mut frame_offset = 0;

    // Iterate through the parsed dataset and log Rerun primitives
    for (idx, ar_frame) in ar_frames.enumerate() {
        let ar_frame = ArFrame::from_raw(
            rec_info.path_ar_frames.parent().unwrap().into(),
            idx,
            ar_frame?,
        );
        let objects = &annotations.objects;
        let annotations = annotations.frame_annotations.as_slice().into();
        log_ar_frame(&mut session, objects, &annotations, &ar_frame)?;
    }

    // TODO(cmc): arg parsing and arg interpretation helpers
    // TODO(cmc): missing flags: save, serve
    // TODO(cmc): expose an easy to use async local mode.
    if args.connect.is_none() {
        let log_messages = session.drain_log_messages_buffer();
        rerun::viewer::show(log_messages)?;
    }

    Ok(())
}

// --- Protobuf parsing ---

#[derive(Debug, Clone)]
struct RecordingInfo {
    path_ar_frames: PathBuf,
    path_annotations: PathBuf,
}

impl Recording {
    fn info(&self) -> anyhow::Result<RecordingInfo> {
        const DATASET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/dataset/downloaded");

        use clap::ValueEnum as _;
        let rec = self.to_possible_value().unwrap();

        // objectron/dataset/downloaded/book
        let path = PathBuf::from(DATASET_DIR).join(rec.get_name());
        // objectron/dataset/downloaded/book/batch-20
        let path = std::fs::read_dir(path)?
            .next()
            .context("empty directory")??
            .path();
        // objectron/dataset/downloaded/book/batch-20/35
        let path = std::fs::read_dir(path)?
            .next()
            .context("empty directory")??
            .path();

        Ok(RecordingInfo {
            // objectron/dataset/downloaded/book/batch-20/35/geometry.pbdata
            path_ar_frames: path.join("geometry.pbdata"),
            // objectron/dataset/downloaded/book/batch-20/35/annotation.pbdata
            path_annotations: path.join("annotation.pbdata"),
        })
    }
}

fn read_ar_frames(path: &Path) -> impl Iterator<Item = anyhow::Result<objectron::ArFrame>> + '_ {
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
    re_log::info!(?path, "reading annotation data");
    let annotations = objectron::Sequence::decode(std::fs::read(path)?.as_slice())?;
    Ok(annotations)
}

mod objectron;
