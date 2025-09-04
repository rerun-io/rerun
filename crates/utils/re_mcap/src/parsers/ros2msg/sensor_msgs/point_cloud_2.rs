use std::io::Cursor;

use super::super::definitions::sensor_msgs::{self, PointField, PointFieldDatatype};
use arrow::{
    array::{
        BooleanBuilder, FixedSizeListBuilder, ListBuilder, StringBuilder, StructBuilder,
        UInt8Builder, UInt32Builder,
    },
    datatypes::{DataType, Field, Fields},
};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt as _};
use re_chunk::{Chunk, ChunkComponents, ChunkId, TimePoint};
use re_log_types::TimeCell;
use re_types::{
    AsComponents as _, Component as _, ComponentDescriptor, SerializedComponentColumn, archetypes,
    components, reflection::ComponentDescriptorExt as _,
};
use std::collections::HashMap;

use super::super::Ros2MessageParser;
use crate::{
    Error,
    parsers::{
        cdr,
        decode::{MessageParser, ParserContext},
        util::{blob_list_builder, fixed_size_list_builder},
    },
};

pub struct PointCloud2MessageParser {
    num_rows: usize,

    height: FixedSizeListBuilder<UInt32Builder>,
    width: FixedSizeListBuilder<UInt32Builder>,
    fields: FixedSizeListBuilder<ListBuilder<StructBuilder>>,
    is_bigendian: FixedSizeListBuilder<BooleanBuilder>,
    point_step: FixedSizeListBuilder<UInt32Builder>,
    row_step: FixedSizeListBuilder<UInt32Builder>,
    data: FixedSizeListBuilder<ListBuilder<UInt8Builder>>,
    is_dense: FixedSizeListBuilder<BooleanBuilder>,

    // We lazily create this, only if we can interpret the point cloud semantically.
    // For now, this is the case if there are fields with names `x`,`y`, and `z` present.
    points_3ds: Option<Vec<archetypes::Points3D>>,
}

impl PointCloud2MessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.PointCloud2";
}

impl Ros2MessageParser for PointCloud2MessageParser {
    fn new(num_rows: usize) -> Self {
        let fields = FixedSizeListBuilder::with_capacity(
            ListBuilder::new(StructBuilder::new(
                Fields::from(vec![
                    Field::new("name", DataType::Utf8, false),
                    Field::new("offset", DataType::UInt32, false),
                    Field::new("datatype", DataType::UInt8, false),
                    Field::new("count", DataType::UInt32, false),
                ]),
                vec![
                    Box::new(StringBuilder::new()),
                    Box::new(UInt32Builder::new()),
                    Box::new(UInt8Builder::new()),
                    Box::new(UInt32Builder::new()),
                ],
            )),
            1,
            num_rows,
        );

        Self {
            num_rows,

            height: fixed_size_list_builder(1, num_rows),
            width: fixed_size_list_builder(1, num_rows),
            fields,
            is_bigendian: fixed_size_list_builder(1, num_rows),
            point_step: fixed_size_list_builder(1, num_rows),
            row_step: fixed_size_list_builder(1, num_rows),
            data: blob_list_builder(num_rows),
            is_dense: fixed_size_list_builder(1, num_rows),

            points_3ds: None,
        }
    }
}

fn access(data: &[u8], datatype: PointFieldDatatype, is_big_endian: bool) -> std::io::Result<f32> {
    let mut rdr = Cursor::new(data);
    match (is_big_endian, datatype) {
        (_, PointFieldDatatype::Unknown) => Ok(0f32), // Not in the original spec.
        (_, PointFieldDatatype::UInt8) => rdr.read_u8().map(|x| x as f32),
        (_, PointFieldDatatype::Int8) => rdr.read_i8().map(|x| x as f32),
        (true, PointFieldDatatype::Int16) => rdr.read_i16::<BigEndian>().map(|x| x as f32),
        (true, PointFieldDatatype::UInt16) => rdr.read_u16::<BigEndian>().map(|x| x as f32),
        (true, PointFieldDatatype::Int32) => rdr.read_i32::<BigEndian>().map(|x| x as f32),
        (true, PointFieldDatatype::UInt32) => rdr.read_u32::<BigEndian>().map(|x| x as f32),
        (true, PointFieldDatatype::Float32) => rdr.read_f32::<BigEndian>(),
        (true, PointFieldDatatype::Float64) => rdr.read_f64::<BigEndian>().map(|x| x as f32),
        (false, PointFieldDatatype::Int16) => rdr.read_i16::<LittleEndian>().map(|x| x as f32),
        (false, PointFieldDatatype::UInt16) => rdr.read_u16::<LittleEndian>().map(|x| x as f32),
        (false, PointFieldDatatype::Int32) => rdr.read_i32::<LittleEndian>().map(|x| x as f32),
        (false, PointFieldDatatype::UInt32) => rdr.read_u32::<LittleEndian>().map(|x| x as f32),
        (false, PointFieldDatatype::Float32) => rdr.read_f32::<LittleEndian>(),
        (false, PointFieldDatatype::Float64) => rdr.read_f64::<LittleEndian>().map(|x| x as f32),
    }
}

pub struct Position3DIter<'a> {
    point_iter: std::slice::ChunksExact<'a, u8>,
    is_big_endian: bool,
    x_accessor: (usize, PointFieldDatatype),
    y_accessor: (usize, PointFieldDatatype),
    z_accessor: (usize, PointFieldDatatype),
}

impl<'a> Position3DIter<'a> {
    fn try_new(
        data: &'a [u8],
        step: usize,
        is_big_endian: bool,
        fields: &[PointField],
    ) -> Option<Self> {
        let mut x_accessor: Option<(usize, PointFieldDatatype)> = None;
        let mut y_accessor: Option<(usize, PointFieldDatatype)> = None;
        let mut z_accessor: Option<(usize, PointFieldDatatype)> = None;

        for field in fields {
            match field.name.as_str() {
                "x" => x_accessor = Some((field.offset as usize, field.datatype)),
                "y" => y_accessor = Some((field.offset as usize, field.datatype)),
                "z" => z_accessor = Some((field.offset as usize, field.datatype)),
                _ => {}
            }
        }

        Some(Self {
            point_iter: data.chunks_exact(step),
            is_big_endian,
            x_accessor: x_accessor?,
            y_accessor: y_accessor?,
            z_accessor: z_accessor?,
        })
    }
}

fn unwrap(res: std::io::Result<f32>, component: &str) -> f32 {
    match res {
        Ok(x) => x,
        Err(err) => {
            debug_assert!(false, "failed to read `{component}`: {err}");
            f32::NAN
        }
    }
}

impl Iterator for Position3DIter<'_> {
    type Item = [f32; 3];

    fn next(&mut self) -> Option<Self::Item> {
        let point = self.point_iter.next()?;

        let x = self.x_accessor;
        let y = self.y_accessor;
        let z = self.z_accessor;

        let x = unwrap(access(&point[x.0..], x.1, self.is_big_endian), "x");
        let y = unwrap(access(&point[y.0..], y.1, self.is_big_endian), "y");
        let z = unwrap(access(&point[z.0..], z.1, self.is_big_endian), "z");

        Some([x, y, z])
    }
}

impl MessageParser for PointCloud2MessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let point_cloud = cdr::try_decode_message::<sensor_msgs::PointCloud2>(msg.data.as_ref())
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        let cell = TimeCell::from_timestamp_nanos_since_epoch(point_cloud.header.stamp.as_nanos());
        ctx.add_time_cell("timestamp", cell);

        let Self {
            num_rows,

            height,
            width,
            fields,
            is_bigendian,
            point_step,
            row_step,
            data,
            is_dense,

            points_3ds,
        } = self;

        let mut timepoint = TimePoint::default();
        timepoint.insert_cell("timestamp", cell);

        height.values().append_slice(&[point_cloud.height]);
        width.values().append_slice(&[point_cloud.width]);

        let position_iter = Position3DIter::try_new(
            &point_cloud.data,
            point_cloud.point_step as usize,
            point_cloud.is_bigendian,
            &point_cloud.fields,
        );

        if let Some(position_iter) = position_iter {
            points_3ds
                .get_or_insert_with(|| Vec::with_capacity(*num_rows))
                .push(archetypes::Points3D::new(position_iter));
        }

        {
            let struct_builder = fields.values();

            for point_field in point_cloud.fields {
                {
                    let name_builder = struct_builder
                        .values()
                        .field_builder::<StringBuilder>(0)
                        .expect("has to exist");
                    name_builder.append_value(point_field.name);
                }
                {
                    let offset_builder = struct_builder
                        .values()
                        .field_builder::<UInt32Builder>(1)
                        .expect("has to exist");
                    offset_builder.append_value(point_field.offset);
                }
                {
                    let datatype_builder = struct_builder
                        .values()
                        .field_builder::<UInt8Builder>(2)
                        .expect("has to exist");
                    datatype_builder.append_value(point_field.datatype as u8);
                }
                {
                    let count_builder = struct_builder
                        .values()
                        .field_builder::<UInt32Builder>(3)
                        .expect("has to exist");
                    count_builder.append_value(point_field.count);
                }
                struct_builder.values().append(true);
            }

            struct_builder.append(true);
            fields.append(true);
        }

        is_bigendian
            .values()
            .append_slice(&[point_cloud.is_bigendian]);
        point_step.values().append_slice(&[point_cloud.point_step]);
        row_step.values().append_slice(&[point_cloud.row_step]);

        data.values().values().append_slice(&point_cloud.data);
        is_dense.values().append_slice(&[point_cloud.is_dense]);

        height.append(true);
        width.append(true);
        is_bigendian.append(true);
        point_step.append(true);
        row_step.append(true);
        is_dense.append(true);

        data.values().append(true);
        data.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            num_rows: _,

            mut width,
            mut height,
            mut fields,
            mut is_bigendian,
            mut point_step,
            mut row_step,
            mut data,
            mut is_dense,

            points_3ds,
        } = *self;

        let mut chunks = Vec::new();

        for (i, points_3d) in points_3ds.into_iter().enumerate() {
            let timelines = timelines
                .iter()
                .map(|(timeline, time_col)| (*timeline, time_col.row_sliced(i, 1).clone()))
                .collect::<HashMap<_, _, _>>();

            let components = points_3d
                .as_serialized_batches()
                .into_iter()
                .map(SerializedComponentColumn::from)
                .collect::<ChunkComponents>();

            let c = Chunk::from_auto_row_ids(
                ChunkId::new(),
                entity_path.clone(),
                timelines,
                components,
            )?;

            chunks.push(c);
        }

        let data_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            [
                (
                    ComponentDescriptor::partial("height")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    height.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("width")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    width.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("fields")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    fields.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("is_bigendian")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    is_bigendian.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("point_step")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    point_step.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("row_step")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    row_step.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("data")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME)
                        .with_component_type(components::Blob::name()),
                    data.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("is_dense")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    is_dense.finish().into(),
                ),
            ]
            .into_iter()
            .collect(),
        )?;

        chunks.push(data_chunk);

        Ok(chunks)
    }
}
