use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, Pinhole};

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::sensor_msgs;
use crate::parsers::ros1msg::wire::Ros1Reader;
use crate::parsers::ros2msg::util::suffix_image_plane_frame_ids;
use crate::util::TimestampCell;

pub struct CameraInfoMessageParser {
    image_from_cameras: Vec<[f32; 9]>,
    resolutions: Vec<(f32, f32)>,
    frame_ids: Vec<String>,
}

impl Ros1MessageParser for CameraInfoMessageParser {
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
        let mut reader = Ros1Reader::new(&msg.data);
        let camera_info = sensor_msgs::CameraInfo::read(&mut reader)?;
        reader.finish()?;

        ctx.add_timestamp_cell(TimestampCell::from_nanos_ros1(
            camera_info.header.stamp.as_nanos(),
            ctx.time_type(),
        ));

        let k = camera_info.k;
        self.image_from_cameras
            .push([k[0], k[3], k[6], k[1], k[4], k[7], k[2], k[5], k[8]].map(|x| x as f32));
        self.resolutions
            .push((camera_info.width as f32, camera_info.height as f32));
        self.frame_ids.push(camera_info.header.frame_id);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let Self {
            image_from_cameras,
            resolutions,
            frame_ids,
        } = *self;

        let image_plane_frame_ids = suffix_image_plane_frame_ids(frame_ids.clone());
        let mut components: Vec<_> = Pinhole::update_fields()
            .with_many_image_from_camera(image_from_cameras)
            .with_many_resolution(resolutions)
            .with_many_parent_frame(frame_ids.clone())
            .with_many_child_frame(image_plane_frame_ids)
            .columns_of_unit_batches()?
            .collect();

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(frame_ids)
                .columns_of_unit_batches()?,
        );

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            ctx.entity_path().clone(),
            ctx.build_timelines(),
            components.into_iter().collect(),
        )?])
    }
}
