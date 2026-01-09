use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, Pinhole};

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs;
use super::super::util::suffix_image_plane_frame_ids;
use crate::Error;
use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};

pub struct CameraInfoMessageParser {
    image_from_cameras: Vec<[f32; 9]>,
    resolutions: Vec<(f32, f32)>,
    frame_ids: Vec<String>,
}

impl Ros2MessageParser for CameraInfoMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            image_from_cameras: Vec::with_capacity(num_rows),
            resolutions: Vec::with_capacity(num_rows),
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for CameraInfoMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let sensor_msgs::CameraInfo {
            header,
            width,
            height,
            k,
            ..
        } = cdr::try_decode_message::<sensor_msgs::CameraInfo>(&msg.data)?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.push(header.frame_id);

        // ROS2 stores the intrinsic matrix K as a row-major 9-element array:
        // [fx, 0, cx, 0, fy, cy, 0, 0, 1]
        // this corresponds to the matrix:
        // [fx,  0, cx]
        // [ 0, fy, cy]
        // [ 0,  0,  1]
        //
        // However, `glam::Mat3` expects column-major data, so we need to transpose
        // the ROS2 row-major data to get the correct matrix layout in Rerun.
        let k_transposed = [
            k[0], k[3], k[6], // first column:  [fx, 0, 0]
            k[1], k[4], k[7], // second column: [0, fy, 0]
            k[2], k[5], k[8], // third column:  [cx, cy, 1]
        ];

        // TODO(#2315): Rerun currently only supports the pinhole model (`plumb_bob` in ROS2)
        // so this does NOT take into account the camera model.
        self.image_from_cameras.push(k_transposed.map(|x| x as f32));
        self.resolutions.push((width as f32, height as f32));

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let Self {
            image_from_cameras,
            resolutions,
            frame_ids,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        // We need a frame ID for the image plane. This doesn't exist in ROS,
        // so we use the camera frame ID with a suffix here (and in the image parsers).
        let image_plane_frame_ids = suffix_image_plane_frame_ids(frame_ids.clone());

        let mut components: Vec<_> = Pinhole::update_fields()
            .with_many_image_from_camera(image_from_cameras)
            .with_many_resolution(resolutions)
            .with_many_parent_frame(frame_ids.clone())
            .with_many_child_frame(image_plane_frame_ids)
            .columns_of_unit_batches()
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?
            .collect();

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(frame_ids)
                .columns_of_unit_batches()
                .map_err(|err| Error::Other(anyhow::anyhow!(err)))?,
        );

        let pinhole_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            components.into_iter().collect(),
        )?;

        Ok(vec![pinhole_chunk])
    }
}
