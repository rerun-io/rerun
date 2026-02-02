use anyhow::Context as _;
use arrow::array::{FixedSizeListArray, FixedSizeListBuilder, Float64Builder};
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, GeoPoints};
use re_sdk_types::components::LatLon;
use re_sdk_types::{ComponentDescriptor, SerializedComponentColumn};

use super::super::Ros2MessageParser;
use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros2msg::definitions::sensor_msgs;
use crate::parsers::util::fixed_size_list_builder;

pub struct NavSatFixMessageParser {
    geo_points: Vec<LatLon>,
    altitude: FixedSizeListBuilder<Float64Builder>,
    frame_ids: Vec<String>,
}

impl NavSatFixMessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.NavSatFix";

    fn create_metadata_column(name: &str, array: FixedSizeListArray) -> SerializedComponentColumn {
        SerializedComponentColumn {
            list_array: array.into(),
            descriptor: ComponentDescriptor::partial(name)
                .with_archetype(Self::ARCHETYPE_NAME.into()),
        }
    }
}

impl Ros2MessageParser for NavSatFixMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            geo_points: Vec::with_capacity(num_rows),
            altitude: fixed_size_list_builder(1, num_rows),
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for NavSatFixMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let sensor_msgs::NavSatFix {
            header,
            latitude,
            longitude,
            altitude,
            ..
        } = cdr::try_decode_message::<sensor_msgs::NavSatFix>(&msg.data)
            .context("Failed to decode sensor_msgs::NavSatFix message from CDR data")?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.push(header.frame_id);

        // Store latitude/longitude as geographic points
        let geo_point = LatLon::new(latitude, longitude);
        self.geo_points.push(geo_point);

        self.altitude.values().append_slice(&[altitude]);
        self.altitude.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            geo_points,
            mut altitude,
            frame_ids,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let mut chunk_components: Vec<_> = GeoPoints::update_fields()
            .with_positions(geo_points)
            .columns_of_unit_batches()?
            .collect();

        chunk_components.extend([Self::create_metadata_column("altitude", altitude.finish())]);

        chunk_components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(frame_ids)
                .columns_of_unit_batches()?,
        );

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            chunk_components.into_iter().collect(),
        )?])
    }
}
