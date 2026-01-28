use std::collections::HashMap;
use std::io::Cursor;

use anyhow::Context as _;
use arrow::array::{
    ArrayBuilder, BooleanBuilder, FixedSizeListBuilder, Float32Builder, Float64Builder,
    Int8Builder, Int16Builder, Int32Builder, ListBuilder, StringBuilder, StructBuilder,
    UInt8Builder, UInt16Builder, UInt32Builder,
};
use arrow::datatypes::{DataType, Field, Fields};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt as _};
use re_chunk::{Chunk, ChunkComponents, ChunkId};
use re_sdk_types::archetypes::CoordinateFrame;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::{
    Archetype as _, AsComponents as _, Component as _, ComponentDescriptor,
    SerializedComponentColumn, archetypes, components,
};

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs::{self, PointField, PointFieldDatatype};
use crate::Error;
use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::util::{blob_list_builder, fixed_size_list_builder};

pub struct PointCloud2MessageParser {
    num_rows: usize,

    frame_id: ListBuilder<StringBuilder>,

    height: FixedSizeListBuilder<UInt32Builder>,
    width: FixedSizeListBuilder<UInt32Builder>,
    fields: FixedSizeListBuilder<ListBuilder<StructBuilder>>,
    is_bigendian: FixedSizeListBuilder<BooleanBuilder>,
    point_step: FixedSizeListBuilder<UInt32Builder>,
    row_step: FixedSizeListBuilder<UInt32Builder>,
    data: FixedSizeListBuilder<ListBuilder<UInt8Builder>>,
    is_dense: FixedSizeListBuilder<BooleanBuilder>,

    extracted_fields: Vec<(String, ListBuilder<Box<dyn ArrayBuilder>>)>,

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

            frame_id: ListBuilder::with_capacity(StringBuilder::new(), num_rows),

            height: fixed_size_list_builder(1, num_rows),
            width: fixed_size_list_builder(1, num_rows),
            fields,
            is_bigendian: fixed_size_list_builder(1, num_rows),
            point_step: fixed_size_list_builder(1, num_rows),
            row_step: fixed_size_list_builder(1, num_rows),
            data: blob_list_builder(num_rows),
            is_dense: fixed_size_list_builder(1, num_rows),

            extracted_fields: Default::default(),

            points_3ds: None,
        }
    }
}

fn builder_from_datatype(datatype: PointFieldDatatype) -> Box<dyn ArrayBuilder> {
    match datatype {
        PointFieldDatatype::Int8 => Box::new(Int8Builder::new()),
        PointFieldDatatype::UInt8 => Box::new(UInt8Builder::new()),
        PointFieldDatatype::Int16 => Box::new(Int16Builder::new()),
        PointFieldDatatype::UInt16 => Box::new(UInt16Builder::new()),
        PointFieldDatatype::Int32 => Box::new(Int32Builder::new()),
        PointFieldDatatype::UInt32 => Box::new(UInt32Builder::new()),
        PointFieldDatatype::Float32 => Box::new(Float32Builder::new()),
        PointFieldDatatype::Float64 => Box::new(Float64Builder::new()),
    }
}

fn access(data: &[u8], datatype: PointFieldDatatype, is_big_endian: bool) -> std::io::Result<f32> {
    let mut rdr = Cursor::new(data);
    match (is_big_endian, datatype) {
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

impl From<PointFieldDatatype> for DataType {
    fn from(value: PointFieldDatatype) -> Self {
        match value {
            PointFieldDatatype::Int8 => Self::Int8,
            PointFieldDatatype::UInt8 => Self::UInt8,
            PointFieldDatatype::Int16 => Self::Int16,
            PointFieldDatatype::UInt16 => Self::UInt16,
            PointFieldDatatype::Int32 => Self::Int32,
            PointFieldDatatype::UInt32 => Self::UInt32,
            PointFieldDatatype::Float32 => Self::Float32,
            PointFieldDatatype::Float64 => Self::Float64,
        }
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

fn add_field_value(
    builder: &mut Box<dyn ArrayBuilder>,
    field: &PointField,
    is_big_endian: bool,
    data: &[u8],
) -> anyhow::Result<()> {
    let mut rdr = Cursor::new(data);
    match field.datatype {
        PointFieldDatatype::Int8 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<Int8Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `Int8Builder`", field.datatype)
                })?;
            let val = rdr.read_i8()?;
            builder.append_value(val);
        }
        PointFieldDatatype::UInt8 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<UInt8Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `UInt8Builder`", field.datatype)
                })?;
            let val = rdr.read_u8()?;
            builder.append_value(val);
        }
        PointFieldDatatype::Int16 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<Int16Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `Int16Builder`", field.datatype)
                })?;
            let val = if is_big_endian {
                rdr.read_i16::<BigEndian>()?
            } else {
                rdr.read_i16::<LittleEndian>()?
            };
            builder.append_value(val);
        }
        PointFieldDatatype::UInt16 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<UInt16Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `UInt16Builder`", field.datatype)
                })?;
            let val = if is_big_endian {
                rdr.read_u16::<BigEndian>()?
            } else {
                rdr.read_u16::<LittleEndian>()?
            };
            builder.append_value(val);
        }

        PointFieldDatatype::Int32 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<Int32Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `Int32Builder`", field.datatype)
                })?;

            let val = if is_big_endian {
                rdr.read_i32::<BigEndian>()?
            } else {
                rdr.read_i32::<LittleEndian>()?
            };
            builder.append_value(val);
        }
        PointFieldDatatype::UInt32 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<UInt32Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `UInt16Builder`", field.datatype)
                })?;
            let val = if is_big_endian {
                rdr.read_u32::<BigEndian>()?
            } else {
                rdr.read_u32::<LittleEndian>()?
            };
            builder.append_value(val);
        }

        PointFieldDatatype::Float32 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<Float32Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `Float32Builder`", field.datatype)
                })?;
            let val = if is_big_endian {
                rdr.read_f32::<BigEndian>()?
            } else {
                rdr.read_f32::<LittleEndian>()?
            };
            builder.append_value(val);
        }

        PointFieldDatatype::Float64 => {
            let builder = builder
                .as_any_mut()
                .downcast_mut::<Float64Builder>()
                .with_context(|| {
                    format!("found datatype {:?}, but `Float64Builder`", field.datatype)
                })?;
            let val = if is_big_endian {
                rdr.read_f64::<BigEndian>()?
            } else {
                rdr.read_f64::<LittleEndian>()?
            };
            builder.append_value(val);
        }
    }

    Ok(())
}

impl MessageParser for PointCloud2MessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let point_cloud = cdr::try_decode_message::<sensor_msgs::PointCloud2>(msg.data.as_ref())
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            point_cloud.header.stamp.as_nanos() as u64,
        ));

        let Self {
            num_rows,

            frame_id,

            height,
            width,
            fields,
            is_bigendian,
            point_step,
            row_step,
            data,
            is_dense,

            extracted_fields,

            points_3ds,
        } = self;

        frame_id.values().append_value(point_cloud.header.frame_id);
        frame_id.append(true);

        height.values().append_slice(&[point_cloud.height]);
        width.values().append_slice(&[point_cloud.width]);

        let position_iter = Position3DIter::try_new(
            &point_cloud.data,
            point_cloud.point_step as usize,
            point_cloud.is_bigendian,
            &point_cloud.fields,
        );

        // We lazily initialize the builders that store the extracted fields from
        // the blob when we receive the first message.
        if extracted_fields.len() != point_cloud.fields.len() {
            *extracted_fields = point_cloud
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        ListBuilder::new(builder_from_datatype(field.datatype)),
                    )
                })
                .collect();
        }

        for point in point_cloud.data.chunks(point_cloud.point_step as usize) {
            for (field, (_name, builder)) in
                point_cloud.fields.iter().zip(extracted_fields.iter_mut())
            {
                let field_builder = builder.values();
                add_field_value(field_builder, field, point_cloud.is_bigendian, point)?;
            }
        }

        for (_name, builder) in extracted_fields {
            builder.append(true);
        }

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

            mut frame_id,
            mut width,
            mut height,
            mut fields,
            mut is_bigendian,
            mut point_step,
            mut row_step,
            mut data,
            mut is_dense,

            extracted_fields: points,

            points_3ds,
        } = *self;

        let mut chunks = Vec::new();

        let frame_ids_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            std::iter::once((CoordinateFrame::descriptor_frame(), frame_id.finish())).collect(),
        )?;
        chunks.push(frame_ids_chunk);

        for (i, points_3d) in points_3ds.iter().flatten().enumerate() {
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
            .chain(points.into_iter().filter_map(|(name, mut builder)| {
                // We only extract additional fields when we have a `Points3d`
                // archetype to attach them to. In that case we're not interested
                // in the other components.
                // TODO(grtlr): It would be nice to never initialize the unnecessary builders
                // in the first place. But, we'll soon move the semantic extraction of `Points3d`
                // into a different layer anyways, making that optimization obsolete.
                points_3ds.as_ref()?;
                if ["x", "y", "z"].contains(&name.as_str()) {
                    None
                } else {
                    Some((
                        ComponentDescriptor::partial(name.clone())
                            .with_builtin_archetype(archetypes::Points3D::name()),
                        builder.finish(),
                    ))
                }
            }))
            .collect(),
        )?;

        chunks.push(data_chunk);

        Ok(chunks)
    }
}
