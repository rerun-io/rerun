use anyhow::Context as _;
use arrow::array::{
    FixedSizeListArray, FixedSizeListBuilder, Float64Builder, Int8Builder, UInt8Builder,
    UInt16Builder,
};
use re_chunk::{Chunk, ChunkId};
use re_log_types::TimeCell;
use re_types::{
    ComponentDescriptor, SerializedComponentColumn, archetypes::GeoPoints, components::LatLon,
};

use super::super::Ros2MessageParser;
use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
    ros2msg::definitions::sensor_msgs,
    util::fixed_size_list_builder,
};

/// Plugin that parses `sensor_msgs/msg/NavSatFix` messages.
#[derive(Default)]
pub struct NavSatFixSchemaPlugin;

pub struct NavSatFixMessageParser {
    geo_points: Vec<LatLon>,
    latitude: FixedSizeListBuilder<Float64Builder>,
    longitude: FixedSizeListBuilder<Float64Builder>,
    altitude: FixedSizeListBuilder<Float64Builder>,
    status: FixedSizeListBuilder<Int8Builder>,
    service: FixedSizeListBuilder<UInt16Builder>,
    position_covariance: FixedSizeListBuilder<Float64Builder>,
    position_covariance_type: FixedSizeListBuilder<UInt8Builder>,
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
            latitude: fixed_size_list_builder(1, num_rows),
            longitude: fixed_size_list_builder(1, num_rows),
            altitude: fixed_size_list_builder(1, num_rows),
            status: fixed_size_list_builder(1, num_rows),
            service: fixed_size_list_builder(1, num_rows),
            position_covariance: fixed_size_list_builder(9, num_rows),
            position_covariance_type: fixed_size_list_builder(1, num_rows),
        }
    }
}

impl MessageParser for NavSatFixMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let sensor_msgs::NavSatFix {
            header,
            status,
            latitude,
            longitude,
            altitude,
            position_covariance,
            position_covariance_type,
        } = cdr::try_decode_message::<sensor_msgs::NavSatFix>(&msg.data)
            .context("Failed to decode sensor_msgs::NavSatFix message from CDR data")?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_time_cell(
            "timestamp",
            TimeCell::from_timestamp_nanos_since_epoch(header.stamp.as_nanos()),
        );

        // Store latitude/longitude as geographic points
        let geo_point = LatLon::new(latitude, longitude);
        self.geo_points.push(geo_point);

        self.latitude.values().append_slice(&[latitude]);
        self.latitude.append(true);

        self.longitude.values().append_slice(&[longitude]);
        self.longitude.append(true);

        self.altitude.values().append_slice(&[altitude]);
        self.altitude.append(true);

        self.status.values().append_slice(&[status.status as i8]);
        self.status.append(true);

        self.service.values().append_slice(&[status.service as u16]);
        self.service.append(true);

        self.position_covariance
            .values()
            .append_slice(&position_covariance);
        self.position_covariance.append(true);

        self.position_covariance_type
            .values()
            .append_slice(&[position_covariance_type as u8]);
        self.position_covariance_type.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            geo_points,
            mut latitude,
            mut longitude,
            mut altitude,
            mut status,
            mut service,
            mut position_covariance,
            mut position_covariance_type,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let mut chunk_components: Vec<_> = GeoPoints::update_fields()
            .with_positions(geo_points)
            .columns_of_unit_batches()?
            .collect();

        chunk_components.extend([
            Self::create_metadata_column("latitude", latitude.finish()),
            Self::create_metadata_column("longitude", longitude.finish()),
            Self::create_metadata_column("altitude", altitude.finish()),
            Self::create_metadata_column("status", status.finish()),
            Self::create_metadata_column("service", service.finish()),
            Self::create_metadata_column("position_covariance", position_covariance.finish()),
            Self::create_metadata_column(
                "position_covariance_type",
                position_covariance_type.finish(),
            ),
        ]);

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            chunk_components.into_iter().collect(),
        )?])
    }
}
