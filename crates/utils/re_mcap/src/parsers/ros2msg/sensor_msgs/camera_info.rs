use super::super::definitions::sensor_msgs;
use arrow::{
    array::{BooleanBuilder, StructBuilder},
    datatypes::Field,
};
use re_chunk::{
    Chunk, ChunkId,
    external::arrow::array::{FixedSizeListBuilder, Float64Builder, StringBuilder, UInt32Builder},
};
use re_log_types::TimeCell;
use re_types::{ComponentDescriptor, archetypes::Pinhole, reflection::ComponentDescriptorExt as _};

use super::super::Ros2MessageParser;
use crate::{
    Error,
    parsers::{
        cdr,
        decode::{MessageParser, ParserContext},
        util::fixed_size_list_builder,
    },
};

/// Plugin that parses `sensor_msgs/msg/CameraInfo` messages.
#[derive(Default)]
pub struct CameraInfoSchemaPlugin;

pub struct CameraInfoMessageParser {
    distortion_models: FixedSizeListBuilder<StringBuilder>,
    k_matrices: FixedSizeListBuilder<Float64Builder>,
    d_coefficients: Vec<Vec<f64>>,
    r_matrices: FixedSizeListBuilder<Float64Builder>,
    p_matrices: FixedSizeListBuilder<Float64Builder>,
    widths: FixedSizeListBuilder<UInt32Builder>,
    heights: FixedSizeListBuilder<UInt32Builder>,
    binning_x: FixedSizeListBuilder<UInt32Builder>,
    binning_y: FixedSizeListBuilder<UInt32Builder>,
    rois: FixedSizeListBuilder<StructBuilder>,
    frame_ids: FixedSizeListBuilder<StringBuilder>,
    image_from_cameras: Vec<[f32; 9]>,
    resolutions: Vec<(f32, f32)>,
}

impl CameraInfoMessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.CameraInfo";
}

impl Ros2MessageParser for CameraInfoMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            distortion_models: fixed_size_list_builder(1, num_rows),
            k_matrices: fixed_size_list_builder(9, num_rows),
            d_coefficients: Vec::with_capacity(num_rows),
            r_matrices: fixed_size_list_builder(9, num_rows),
            p_matrices: fixed_size_list_builder(12, num_rows),
            widths: fixed_size_list_builder(1, num_rows),
            heights: fixed_size_list_builder(1, num_rows),
            binning_x: fixed_size_list_builder(1, num_rows),
            binning_y: fixed_size_list_builder(1, num_rows),
            rois: FixedSizeListBuilder::with_capacity(
                StructBuilder::new(
                    vec![
                        Field::new("x_offset", arrow::datatypes::DataType::UInt32, false),
                        Field::new("y_offset", arrow::datatypes::DataType::UInt32, false),
                        Field::new("width", arrow::datatypes::DataType::UInt32, false),
                        Field::new("height", arrow::datatypes::DataType::UInt32, false),
                        Field::new("do_rectify", arrow::datatypes::DataType::Boolean, false),
                    ],
                    vec![
                        Box::new(UInt32Builder::new()),
                        Box::new(UInt32Builder::new()),
                        Box::new(UInt32Builder::new()),
                        Box::new(UInt32Builder::new()),
                        Box::new(BooleanBuilder::new()),
                    ],
                ),
                1,
                num_rows,
            ),
            frame_ids: fixed_size_list_builder(1, num_rows),
            image_from_cameras: Vec::with_capacity(num_rows),
            resolutions: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for CameraInfoMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let sensor_msgs::CameraInfo {
            header,
            width,
            height,
            distortion_model,
            d,
            k,
            r,
            p,
            binning_x,
            binning_y,
            roi,
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

        self.r_matrices.values().append_slice(&r);
        self.r_matrices.append(true);

        self.p_matrices.values().append_slice(&p);
        self.p_matrices.append(true);

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

        let struct_builder = self.rois.values();

        struct_builder
            .field_builder::<UInt32Builder>(0)
            .expect("has to exist")
            .append_value(roi.x_offset);

        struct_builder
            .field_builder::<UInt32Builder>(1)
            .expect("has to exist")
            .append_value(roi.y_offset);

        struct_builder
            .field_builder::<UInt32Builder>(2)
            .expect("has to exist")
            .append_value(roi.width);

        struct_builder
            .field_builder::<UInt32Builder>(3)
            .expect("has to exist")
            .append_value(roi.height);

        struct_builder
            .field_builder::<BooleanBuilder>(4)
            .expect("has to exist")
            .append_value(roi.do_rectify);

        struct_builder.append(true);
        self.rois.append(true);

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
            mut r_matrices,
            mut p_matrices,
            d_coefficients,
            mut widths,
            mut heights,
            mut binning_x,
            mut binning_y,
            mut frame_ids,
            mut rois,
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
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    distortion_models.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("k").with_builtin_archetype(Self::ARCHETYPE_NAME),
                    k_matrices.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("width")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    widths.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("height")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    heights.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("d").with_builtin_archetype(Self::ARCHETYPE_NAME),
                    d_array,
                ),
                (
                    ComponentDescriptor::partial("r").with_builtin_archetype(Self::ARCHETYPE_NAME),
                    r_matrices.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("p").with_builtin_archetype(Self::ARCHETYPE_NAME),
                    p_matrices.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("binning_x")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    binning_x.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("binning_y")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    binning_y.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("roi")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    rois.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("frame_id")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    frame_ids.finish().into(),
                ),
            ]
            .into_iter()
            .collect(),
        )?;

        let pinhole_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            Pinhole::update_fields()
                .with_many_image_from_camera(image_from_cameras)
                .with_many_resolution(resolutions)
                .columns_of_unit_batches()
                .map_err(|err| Error::Other(anyhow::anyhow!(err)))?
                .collect(),
        )?;

        Ok(vec![chunk, pinhole_chunk])
    }
}
