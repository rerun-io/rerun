use std::{collections::BTreeMap, sync::Arc};

use arrow::{
    array::{
        ArrayBuilder, ArrayRef, BinaryArray, BinaryBuilder, BooleanArray, BooleanBuilder,
        FixedSizeListBuilder, Float32Array, Float32Builder, Float64Array, Float64Builder,
        GenericListArray, Int32Array, Int32Builder, Int64Array, Int64Builder, ListBuilder,
        MapBuilder, NullArray, NullBuilder, OffsetSizeTrait, StringArray, StringBuilder,
        StructBuilder, UInt32Array, UInt32Builder, UInt64Array, UInt64Builder, UnionBuilder,
    },
    datatypes::{DataType, Field, Fields, UnionFields, UnionMode},
};
use mcap::Schema;
use prost_reflect::{
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, Value,
};
use re_chunk::{Chunk, ChunkId};
use re_types::{ComponentDescriptor, Loggable};

use crate::mcap::decode::{McapMessageParser, PluginError};

pub struct ProtobufMessageParser {
    num_rows: usize,
    message_descriptor: MessageDescriptor,
    fields: BTreeMap<String, FixedSizeListBuilder<Box<dyn ArrayBuilder>>>,
    archetype: String,
}

impl ProtobufMessageParser {
    // TODO: Store the descriptor pool somewhere to avoid doing the thing on every message!
    pub fn new(num_rows: usize, schema: &Arc<Schema<'_>>) -> Self {
        let pool = DescriptorPool::decode(schema.data.as_ref()).unwrap();

        let message_descriptor = dbg!(pool.get_message_by_name(schema.name.as_str()).unwrap());

        let mut fields = BTreeMap::new();

        // We build up the Arrow builders for this particular message.
        for field_descr in message_descriptor.fields() {
            let name = field_descr.name().to_owned();
            let builder = arrow_builder_from_field(&field_descr);
            fields.insert(
                name,
                FixedSizeListBuilder::with_capacity(builder, 1, num_rows),
            );
            re_log::debug!("Added Arrow builder for fields: {}", field_descr.name());
        }

        debug_assert!(
            message_descriptor.oneofs().len() == 0,
            "`oneof` is not supported yet"
        );

        Self {
            num_rows,
            message_descriptor,
            fields,
            archetype: schema.name.clone(),
        }
    }
}

impl McapMessageParser for ProtobufMessageParser {
    fn append(
        &mut self,
        ctx: &mut crate::mcap::decode::ParserContext,
        msg: &mcap::Message<'_>,
    ) -> anyhow::Result<()> {
        let dynamic_message =
            DynamicMessage::decode(self.message_descriptor.clone(), msg.data.as_ref()).unwrap();

        for (field_descr, val) in dynamic_message.fields() {
            re_log::debug!("Field {}: Start writing to builders", field_descr.name());
            let Some(rows_builder) = self.fields.get_mut(field_descr.name()) else {
                re_log::error_once!(
                    "Message has field that is not part of its definition: {}",
                    field_descr.name()
                );
                continue;
            };

            let is_valid = append_value(rows_builder.values(), val).is_some();
            rows_builder.append(is_valid);
            re_log::debug!(
                "Field {}: Finished writing to builders; success: {is_valid}",
                field_descr.name(),
            );
        }

        Ok(())
    }

    fn finalize(
        self: Box<Self>,
        ctx: crate::mcap::decode::ParserContext,
    ) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            num_rows: _,
            message_descriptor: _,
            fields,
            archetype,
        } = *self;

        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            fields
                .into_iter()
                .map(|(field, mut builder)| {
                    (
                        ComponentDescriptor::partial(field)
                            .with_archetype(archetype.as_str().into()),
                        builder.finish().into(),
                    )
                })
                .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        Ok(vec![message_chunk])
    }
}

macro_rules! append_primitive {
    ($builder_type:ty, $builder:expr, $value:expr ) => {{
        let builder = $builder
            .as_any_mut()
            .downcast_mut::<$builder_type>()
            .unwrap();
        builder.append_value($value);
    }};
}
// TODO: proper errors!
fn append_value(builder: &mut dyn ArrayBuilder, val: &Value) -> Option<()> {
    // re_log::debug!("Called append on {val}");
    match val {
        Value::Bool(x) => append_primitive!(BooleanBuilder, builder, *x),
        Value::I32(x) => append_primitive!(Int32Builder, builder, *x),
        Value::I64(x) => append_primitive!(Int64Builder, builder, *x),
        Value::U32(x) => append_primitive!(UInt32Builder, builder, *x),
        Value::U64(x) => append_primitive!(UInt64Builder, builder, *x),
        Value::F32(x) => append_primitive!(Float32Builder, builder, *x),
        Value::F64(x) => append_primitive!(Float64Builder, builder, *x),
        Value::String(x) => append_primitive!(StringBuilder, builder, x.clone()),
        Value::Bytes(bytes) => append_primitive!(BinaryBuilder, builder, bytes.clone()),
        Value::Message(dynamic_message) => {
            re_log::debug!(
                "Append called on dynamic message with fields: {:?}",
                dynamic_message
                    .fields()
                    .map(|(descr, _)| descr.name().to_owned())
                    .collect::<Vec<_>>()
            );
            // let list_builder = builder
            //     .as_any_mut()
            //     .downcast_mut::<ListBuilder<Box<dyn ArrayBuilder>>>()
            //     .unwrap();
            let struct_builder = builder
                .as_any_mut()
                .downcast_mut::<StructBuilder>()
                .unwrap();
            re_log::debug!(
                "Retrieved StructBuilder with {} fields",
                struct_builder.num_fields()
            );

            for (ith_arrow_field, field_builder) in
                struct_builder.field_builders_mut().iter_mut().enumerate()
            {
                // Protobuf fields are 1-indexed, so we need to map the i-th builder.
                let protobuf_number = ith_arrow_field as u32 + 1;
                if let Some(val) = dynamic_message.get_field_by_number(protobuf_number) {
                    let is_valid = append_value(field_builder, val.as_ref());
                    re_log::debug!(
                        "Written field ({protobuf_number}) with val: {val} -- success: {is_valid:?}"
                    );
                } else {
                    re_log::warn!("Missing field {ith_arrow_field}, appending null");
                    //field_builder.append_null();
                }
            }
            // for (descr, val) in dynamic_message.fields() {
            //     // In protobuf, fields are not 0-indexed, so we need to subtract 1.
            //     let i = descr.number() as usize;
            //     if let Some(field_builder) = builders.get_mut(i - 1) {
            //     } else {
            //         re_log::error!("Missing builder for field: {}", descr.name());
            //     }
            // }

            // for (builder, (descr, val)) in struct_builder
            //     .field_builders_mut()
            //     .iter_mut()
            //     .zip(dynamic_message.fields())
            // {
            //     let is_valid = append_value(builder, val).is_some();
            //     re_log::debug!("Nested field {}: success {is_valid}", descr.name());
            // }
            struct_builder.append(true);
            // list_builder.append(true);
        }
        Value::List(vec) => {
            re_log::debug!("Append called on a list with {} elements: {val}", vec.len(),);
            let list_builder = builder
                .as_any_mut()
                .downcast_mut::<ListBuilder<Box<dyn ArrayBuilder>>>()
                .unwrap();

            let is_valid = vec
                .iter()
                .all(|v| append_value(list_builder.values(), v).is_some());
            list_builder.append(is_valid);
            re_log::debug!("Finished append on list with elements {val}");
        }
        Value::Map(hash_map) => {
            re_log::error_once!("Cannot build map yet");
            return None;
        }
        Value::EnumNumber(x) => {
            let enum_builder = builder.as_any_mut().downcast_mut::<Int32Builder>().unwrap();
            enum_builder.append_value(*x);
            // enum_builder.append_null();
        }
    }

    Some(())
}

fn struct_builder_from_message(message_descriptor: &MessageDescriptor) -> StructBuilder {
    let fields = message_descriptor
        .fields()
        .map(|f| arrow_field_from(&f))
        .collect::<Fields>();
    let field_builders = message_descriptor
        .fields()
        .map(|f| arrow_builder_from_field(&f))
        .collect::<Vec<_>>();

    debug_assert_eq!(fields.len(), field_builders.len());

    re_log::debug!(
        "Created StructBuilder for message {} with fields: {:?}",
        message_descriptor.full_name(),
        fields.iter().map(|f| f.name()).collect::<Vec<_>>()
    );
    StructBuilder::new(fields, field_builders)
}

fn arrow_builder_from_field(descr: &FieldDescriptor) -> Box<dyn ArrayBuilder> {
    let inner: (&'static str, Box<dyn ArrayBuilder>) = match descr.kind() {
        Kind::Double => ("F64", Box::new(Float64Builder::new())),
        Kind::Float => ("F32", Box::new(Float32Builder::new())),
        Kind::Int32 | Kind::Sfixed32 | Kind::Sint32 => ("Int32", Box::new(Int32Builder::new())),
        Kind::Int64 | Kind::Sfixed64 | Kind::Sint64 => ("Int64", Box::new(Int64Builder::new())),
        Kind::Uint32 | Kind::Fixed32 => ("Uint32", Box::new(UInt32Builder::new())),
        Kind::Uint64 | Kind::Fixed64 => ("Uint64", Box::new(UInt64Builder::new())),
        Kind::Bool => ("Bool", Box::new(BooleanBuilder::new())),
        Kind::String => ("String", Box::new(StringBuilder::new())),
        Kind::Bytes => ("Binary", Box::new(BinaryBuilder::new())),
        Kind::Message(message_descriptor) => (
            "Struct",
            Box::new(struct_builder_from_message(&message_descriptor)) as Box<dyn ArrayBuilder>,
        ),
        Kind::Enum(_) => {
            // TODO(grtlr): Use proper Union type here!
            ("Enum(Int32)", Box::new(Int32Builder::new()))
        }
    };

    if descr.is_list() {
        re_log::error!("ListBuilder({}Builder): {descr:?}", inner.0);
        return Box::new(ListBuilder::new(inner.1));
    }

    re_log::error!("{}Builder: {}", inner.0, descr.name());
    inner.1
}

fn arrow_field_from(descr: &FieldDescriptor) -> Field {
    Field::new(descr.name(), datatype_from(descr), true)
}

fn datatype_from(descr: &FieldDescriptor) -> DataType {
    let inner = match descr.kind() {
        Kind::Double => DataType::Float64,
        Kind::Float => DataType::Float32,
        Kind::Int32 | Kind::Sfixed32 | Kind::Sint32 => DataType::Int32,
        Kind::Int64 | Kind::Sfixed64 | Kind::Sint64 => DataType::Int64,
        Kind::Uint32 | Kind::Fixed32 => DataType::UInt32,
        Kind::Uint64 | Kind::Fixed64 => DataType::UInt64,
        Kind::Bool => DataType::Boolean,
        Kind::String => DataType::Utf8,
        Kind::Bytes => DataType::Binary,
        Kind::Message(message_descriptor) => {
            let fields = message_descriptor
                .fields()
                .map(|f| arrow_field_from(&f))
                .collect::<Fields>();
            // DataType::List(Arc::new(Field::new_list_field(
            //     DataType::Struct(fields),
            //     true,
            // )))
            DataType::Struct(fields)
        }
        Kind::Enum(enum_descriptor) => {
            // re_log::warn!("{enum_descriptor:#?}");
            // let type_ids = enum_descriptor
            //     .values()
            //     .into_iter()
            //     .map(|val_descr| val_descr.number() as i8);
            // let fields = enum_descriptor
            //     .values()
            //     .into_iter()
            //     .map(|val_descr| Field::new(val_descr.name(), DataType::Utf8, false));
            // DataType::Union(UnionFields::new(type_ids, fields), UnionMode::Dense)
            DataType::Int32
        }
    };

    if descr.is_list() {
        return DataType::new_list(inner, true);
    }

    inner
}
