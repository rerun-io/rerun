use re_chunk::{
    Chunk, ChunkId,
    external::arrow::array::{FixedSizeListBuilder, Float64Builder, StringBuilder, UInt32Builder},
};
use re_log_types::TimeCell;
use re_mcap_ros2::sensor_msgs;
use re_types::{ComponentDescriptor, archetypes::Pinhole};

use crate::mcap::{
    cdr,
    decode::{McapMessageParser, ParserContext, PluginError, SchemaName, SchemaPlugin},
    schema::fixed_size_list_builder,
};

/// Plugin that parses `sensor_msgs/msg/CameraInfo` messages.
#[derive(Default)]
pub struct CameraInfoSchemaPlugin;

impl SchemaPlugin for CameraInfoSchemaPlugin {
    fn name(&self) -> SchemaName {
        "sensor_msgs/msg/CameraInfo".into()
    }

    fn create_message_parser(
        &self,
        _channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Box<dyn McapMessageParser> {
        Box::new(CameraInfoMessageParser::new(num_rows)) as Box<dyn McapMessageParser>
    }
}

pub struct CameraInfoMessageParser {
    distortion_models: FixedSizeListBuilder<StringBuilder>,
    k_matrices: FixedSizeListBuilder<Float64Builder>,
    d_coefficients: Vec<Vec<f64>>,
    widths: FixedSizeListBuilder<UInt32Builder>,
    heights: FixedSizeListBuilder<UInt32Builder>,
    binning_x: FixedSizeListBuilder<UInt32Builder>,
    binning_y: FixedSizeListBuilder<UInt32Builder>,
    frame_ids: FixedSizeListBuilder<StringBuilder>,
    image_from_cameras: Vec<[f32; 9]>,
    resolutions: Vec<(f32, f32)>,
}

impl CameraInfoMessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.CameraInfo";

    pub fn new(num_rows: usize) -> Self {
        Self {
            distortion_models: fixed_size_list_builder(1, num_rows),
            k_matrices: fixed_size_list_builder(9, num_rows),
            d_coefficients: Vec::with_capacity(num_rows),
            widths: fixed_size_list_builder(1, num_rows),
            heights: fixed_size_list_builder(1, num_rows),
            binning_x: fixed_size_list_builder(1, num_rows),
            binning_y: fixed_size_list_builder(1, num_rows),
            frame_ids: fixed_size_list_builder(1, num_rows),
            image_from_cameras: Vec::with_capacity(num_rows),
            resolutions: Vec::with_capacity(num_rows),
        }
    }
}

impl McapMessageParser for CameraInfoMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let sensor_msgs::CameraInfo {
            header,
            width,
            height,
            distortion_model,
            d,
            k,
            r: _,
            p: _,
            binning_x,
            binning_y,
            roi: _,
        } = cdr::try_decode_message::<sensor_msgs::CameraInfo>(&msg.data)?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_time_cell(
            "timestamp",
            TimeCell::from_timestamp_nanos_since_epoch(header.stamp.as_nanos()),
        );

        self.distortion_models
            .values()
            .append_value(&distortion_model);
        self.distortion_models.append(true);
        self.k_matrices.values().append_slice(&k);
        self.k_matrices.append(true);

        self.d_coefficients.push(d);

        self.widths.values().append_value(width);
        self.widths.append(true);

        self.heights.values().append_value(height);
        self.heights.append(true);

        self.binning_x.values().append_value(binning_x);
        self.binning_x.append(true);

        self.binning_y.values().append_value(binning_y);
        self.binning_y.append(true);

        self.frame_ids.values().append_value(&header.frame_id);
        self.frame_ids.append(true);

        // TODO(#2315): Rerun currently only supports the pinhole model (`plumb_bob` in ROS2)
        // so this does NOT take into account the camera model.
        self.image_from_cameras.push(k.map(|x| x as f32));
        self.resolutions.push((width as f32, height as f32));

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let Self {
            mut distortion_models,
            mut k_matrices,
            d_coefficients,
            mut widths,
            mut heights,
            mut binning_x,
            mut binning_y,
            mut frame_ids,
            image_from_cameras,
            resolutions,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let d_array = {
            let mut list_builder = arrow::array::ListBuilder::new(Float64Builder::new());
            for d_vec in d_coefficients {
                list_builder.values().append_slice(&d_vec);
                list_builder.append(true);
            }
            list_builder.finish()
        };

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            [
                (
                    ComponentDescriptor::partial("distortion_model")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    distortion_models.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("k").with_archetype(Self::ARCHETYPE_NAME.into()),
                    k_matrices.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("width")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    widths.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("height")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    heights.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("d").with_archetype(Self::ARCHETYPE_NAME.into()),
                    d_array,
                ),
                (
                    ComponentDescriptor::partial("binning_x")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    binning_x.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("binning_y")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    binning_y.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("frame_id")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    frame_ids.finish().into(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        let pinhole_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            Pinhole::update_fields()
                .with_many_image_from_camera(image_from_cameras)
                .with_many_resolution(resolutions)
                .columns_of_unit_batches()
                .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?
                .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        Ok(vec![chunk, pinhole_chunk])
    }
}
