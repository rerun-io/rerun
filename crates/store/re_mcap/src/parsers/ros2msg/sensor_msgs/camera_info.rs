use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, Pinhole};

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs;
use super::super::util::spatial_camera_frame_ids_or_log_missing;
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
        ctx.add_timestamp_cell(crate::util::TimestampCell::from_nanos_ros2(
            header.stamp.as_nanos() as u64,
            ctx.time_type(),
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
        let Some(frame_ids) = spatial_camera_frame_ids_or_log_missing(
            ctx.channel_topic(),
            &entity_path,
            "sensor_msgs/msg/CameraInfo",
            "Skipping camera calibration import for this topic.",
            frame_ids,
        ) else {
            return Ok(Vec::new());
        };
        let timelines = ctx.build_timelines();

        let mut components: Vec<_> = Pinhole::update_fields()
            .with_many_image_from_camera(image_from_cameras)
            .with_many_resolution(resolutions)
            .with_many_parent_frame(frame_ids.camera_frame_ids.clone())
            .with_many_child_frame(frame_ids.image_plane_frame_ids)
            .columns_of_unit_batches()
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?
            .collect();

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(frame_ids.camera_frame_ids)
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

#[cfg(test)]
mod tests {
    use re_chunk::EntityPath;
    use re_log_types::TimeType;

    use super::*;

    fn test_ctx() -> ParserContext {
        ParserContext::new(
            EntityPath::from("/tests/camera_info"),
            "/tests/camera_info",
            TimeType::TimestampNs,
        )
    }

    fn install_warn_logger() -> re_log::Receiver<re_log::LogMsg> {
        re_log::setup_logging();
        let (logger, log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Warn);
        re_log::add_boxed_logger(Box::new(logger)).expect("Failed to add logger");
        log_rx
    }

    #[track_caller]
    fn expect_single_matching_warning(
        log_rx: &re_log::Receiver<re_log::LogMsg>,
        expected_substring: &str,
    ) {
        let matching_logs = std::iter::from_fn(|| log_rx.try_recv().ok())
            .filter(|log| log.level == re_log::Level::Warn && log.msg.contains(expected_substring))
            .collect::<Vec<_>>();
        assert_eq!(
            matching_logs.len(),
            1,
            "Expected exactly one matching warning containing {expected_substring:?}, got {matching_logs:?}"
        );
    }

    #[test]
    fn drops_camera_info_topic_when_any_frame_id_is_missing() {
        let log_rx = install_warn_logger();

        let parser = CameraInfoMessageParser {
            image_from_cameras: vec![[1.0; 9], [2.0; 9]],
            resolutions: vec![(640.0, 480.0), (640.0, 480.0)],
            frame_ids: vec!["camera".to_owned(), "   ".to_owned()],
        };

        let chunks = Box::new(parser).finalize(test_ctx()).unwrap();

        assert!(chunks.is_empty());
        expect_single_matching_warning(&log_rx, "Skipping camera calibration import");
    }

    #[test]
    fn keeps_spatial_components_for_camera_info_with_valid_frame_ids() {
        let parser = CameraInfoMessageParser {
            image_from_cameras: vec![[1.0; 9]],
            resolutions: vec![(640.0, 480.0)],
            frame_ids: vec!["camera".to_owned()],
        };

        let chunks = Box::new(parser).finalize(test_ctx()).unwrap();
        let chunk = chunks.first().unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(
            chunk
                .components()
                .contains_component(Pinhole::descriptor_parent_frame().component)
        );
        assert!(
            chunk
                .components()
                .contains_component(Pinhole::descriptor_child_frame().component)
        );
        assert!(
            chunk
                .components()
                .contains_component(CoordinateFrame::descriptor_frame().component)
        );
    }
}
