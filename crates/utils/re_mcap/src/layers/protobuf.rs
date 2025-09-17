use std::collections::BTreeMap;

use arrow::{
    array::{
        ArrayBuilder, BinaryBuilder, BooleanBuilder, FixedSizeListBuilder, Float32Builder,
        Float64Builder, Int32Builder, Int64Builder, ListBuilder, StringBuilder, StructBuilder,
        UInt32Builder, UInt64Builder,
    },
    datatypes::{DataType, Field, Fields},
};
use prost_reflect::{
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, Value,
};
use re_chunk::{Chunk, ChunkId};
use re_types::{ComponentDescriptor, reflection::ComponentDescriptorExt as _};

use crate::parsers::{MessageParser, ParserContext};
use crate::{Error, LayerIdentifier, MessageLayer};

struct ProtobufMessageParser {
    message_descriptor: MessageDescriptor,
    fields: BTreeMap<String, FixedSizeListBuilder<Box<dyn ArrayBuilder>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtobufError {
    #[error("invalid message on channel {channel} for schema {schema}: {source}")]
    InvalidMessage {
        schema: String,
        channel: String,
        source: prost_reflect::prost::DecodeError,
    },

    #[error("expected type {expected_type}, but found value {value}")]
    UnexpectedValue {
        expected_type: &'static str,
        value: Value,
    },

    #[error("type {0} is not supported yyet")]
    UnsupportedType(&'static str),

    #[error("missing protobuf field {field}")]
    MissingField { field: u32 },
}

impl ProtobufMessageParser {
    fn new(num_rows: usize, message_descriptor: MessageDescriptor) -> Self {
        let mut fields = BTreeMap::new();

        // We recursively build up the Arrow builders for this particular message.
        for field_descr in message_descriptor.fields() {
            let name = field_descr.name().to_owned();
            let builder = arrow_builder_from_field(&field_descr);
            fields.insert(
                name,
                FixedSizeListBuilder::with_capacity(builder, 1, num_rows),
            );
            re_log::trace!("Added Arrow builder for fields: {}", field_descr.name());
        }

        if message_descriptor.oneofs().len() > 0 {
            re_log::warn_once!(
                "`oneof` in schema {} is not supported yet.",
                message_descriptor.full_name()
            );
            debug_assert!(
                message_descriptor.oneofs().len() == 0,
                "`oneof` in schema {} is not supported yet",
                message_descriptor.full_name()
            );
        }

        Self {
            message_descriptor,
            fields,
        }
    }
}

impl MessageParser for ProtobufMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let dynamic_message =
            DynamicMessage::decode(self.message_descriptor.clone(), msg.data.as_ref()).map_err(
                |err| ProtobufError::InvalidMessage {
                    schema: self.message_descriptor.full_name().to_owned(),
                    channel: msg.channel.topic.clone(),
                    source: err,
                },
            )?;

        // We always need to make sure to iterate over all our builders, adding null values whenever
        // a field is missing from the message that we received.
        for (field, builder) in &mut self.fields {
            if let Some(val) = dynamic_message.get_field_by_name(field.as_str()) {
                append_value(builder.values(), val.as_ref())?;
                builder.append(true);
                re_log::trace!("Field {}: Finished writing to builders", field);
            } else {
                builder.append(false);
            }
        }

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            message_descriptor,
            fields,
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
                            .with_builtin_archetype(message_descriptor.full_name()),
                        builder.finish().into(),
                    )
                })
                .collect(),
        )
        .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        Ok(vec![message_chunk])
    }
}

fn downcast_err<'a, T: std::any::Any>(
    builder: &'a mut dyn ArrayBuilder,
    val: &Value,
) -> Result<&'a mut T, ProtobufError> {
    builder.as_any_mut().downcast_mut::<T>().ok_or_else(|| {
        let type_name = std::any::type_name::<T>();
        ProtobufError::UnexpectedValue {
            expected_type: type_name.strip_suffix("Builder").unwrap_or(type_name),
            value: val.clone(),
        }
    })
}

fn append_value(builder: &mut dyn ArrayBuilder, val: &Value) -> Result<(), ProtobufError> {
    match val {
        Value::Bool(x) => downcast_err::<BooleanBuilder>(builder, val)?.append_value(*x),
        Value::I32(x) => downcast_err::<Int32Builder>(builder, val)?.append_value(*x),
        Value::I64(x) => downcast_err::<Int64Builder>(builder, val)?.append_value(*x),
        Value::U32(x) => downcast_err::<UInt32Builder>(builder, val)?.append_value(*x),
        Value::U64(x) => downcast_err::<UInt64Builder>(builder, val)?.append_value(*x),
        Value::F32(x) => downcast_err::<Float32Builder>(builder, val)?.append_value(*x),
        Value::F64(x) => downcast_err::<Float64Builder>(builder, val)?.append_value(*x),
        Value::String(x) => downcast_err::<StringBuilder>(builder, val)?.append_value(x.clone()),
        Value::Bytes(bytes) => {
            downcast_err::<BinaryBuilder>(builder, val)?.append_value(bytes.clone());
        }
        Value::Message(dynamic_message) => {
            re_log::trace!(
                "Append called on dynamic message with fields: {:?}",
                dynamic_message
                    .fields()
                    .map(|(descr, _)| descr.name().to_owned())
                    .collect::<Vec<_>>()
            );
            let struct_builder = downcast_err::<StructBuilder>(builder, val)?;
            re_log::trace!(
                "Retrieved StructBuilder with {} fields",
                struct_builder.num_fields()
            );

            for (ith_arrow_field, field_builder) in
                struct_builder.field_builders_mut().iter_mut().enumerate()
            {
                // Protobuf fields are 1-indexed, so we need to map the i-th builder.
                let protobuf_number = ith_arrow_field as u32 + 1;
                let val = dynamic_message
                    .get_field_by_number(protobuf_number)
                    .ok_or_else(|| ProtobufError::MissingField {
                        field: protobuf_number,
                    })?;
                re_log::trace!("Written field ({protobuf_number}) with val: {val}");
                append_value(field_builder, val.as_ref())?;
            }
            struct_builder.append(true);
        }
        Value::List(vec) => {
            re_log::trace!("Append called on a list with {} elements: {val}", vec.len(),);
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;

            for val in vec {
                append_value(list_builder.values(), val)?;
            }
            list_builder.append(true);
            re_log::trace!("Finished append on list with elements {val}");
        }
        Value::Map(_hash_map) => {
            // We should not encounter hash maps in protobufs.
            return Err(ProtobufError::UnsupportedType("HashMap"));
        }
        Value::EnumNumber(x) => {
            // Change this to a `UnionBuilder`:
            // https://github.com/apache/arrow-rs/issues/8033
            downcast_err::<Int32Builder>(builder, val)?.append_value(*x);
        }
    }

    Ok(())
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

    re_log::trace!(
        "Created StructBuilder for message {} with fields: {:?}",
        message_descriptor.full_name(),
        fields.iter().map(|f| f.name()).collect::<Vec<_>>()
    );
    StructBuilder::new(fields, field_builders)
}

fn arrow_builder_from_field(descr: &FieldDescriptor) -> Box<dyn ArrayBuilder> {
    let inner: Box<dyn ArrayBuilder> = match descr.kind() {
        Kind::Double => Box::new(Float64Builder::new()),
        Kind::Float => Box::new(Float32Builder::new()),
        Kind::Int32 | Kind::Sfixed32 | Kind::Sint32 => Box::new(Int32Builder::new()),
        Kind::Int64 | Kind::Sfixed64 | Kind::Sint64 => Box::new(Int64Builder::new()),
        Kind::Uint32 | Kind::Fixed32 => Box::new(UInt32Builder::new()),
        Kind::Uint64 | Kind::Fixed64 => Box::new(UInt64Builder::new()),
        Kind::Bool => Box::new(BooleanBuilder::new()),
        Kind::String => Box::new(StringBuilder::new()),
        Kind::Bytes => Box::new(BinaryBuilder::new()),
        Kind::Message(message_descriptor) => {
            Box::new(struct_builder_from_message(&message_descriptor)) as Box<dyn ArrayBuilder>
        }
        Kind::Enum(_) => {
            re_log::warn_once!(
                "Enum support is still limited, falling back to Int32 representation"
            );
            Box::new(Int32Builder::new())
        }
    };

    if descr.is_list() {
        return Box::new(ListBuilder::new(inner));
    }

    inner
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
            DataType::Struct(fields)
        }
        Kind::Enum(_) => {
            // TODO(apache/arrow-rs#8033): Implement enum support when `UnionBuilder` implements `ArrayBuilder`.
            DataType::Int32
        }
    };

    if descr.is_list() {
        return DataType::new_list(inner, true);
    }

    inner
}

/// Provides reflection-based conversion of protobuf-encoded MCAP messages.
///
/// Applying this layer will result in a direct Arrow representation of the fields.
/// This is useful for querying certain fields from an MCAP file, but wont result
/// in semantic types that can be picked up by the Rerun viewer.
#[derive(Debug, Default)]
pub struct McapProtobufLayer {
    descrs_per_topic: ahash::HashMap<String, MessageDescriptor>,
}

impl MessageLayer for McapProtobufLayer {
    fn identifier() -> LayerIdentifier {
        "protobuf".into()
    }

    fn init(&mut self, summary: &mcap::Summary) -> Result<(), Error> {
        for channel in summary.channels.values() {
            let schema = channel
                .schema
                .as_ref()
                .ok_or(Error::NoSchema(channel.topic.clone()))?;

            if schema.encoding.as_str() != "protobuf" {
                continue;
            }

            let pool = DescriptorPool::decode(schema.data.as_ref()).map_err(|err| {
                Error::InvalidSchema {
                    schema: schema.name.clone(),
                    source: err.into(),
                }
            })?;

            let message_descriptor = pool
                .get_message_by_name(schema.name.as_str())
                .ok_or_else(|| Error::NoSchema(schema.name.clone()))?;

            let found = self
                .descrs_per_topic
                .insert(channel.topic.clone(), message_descriptor);
            debug_assert!(found.is_none());
        }

        Ok(())
    }

    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        let Some(schema) = channel.schema.as_ref() else {
            return false;
        };

        if schema.encoding.as_str() != "protobuf" {
            return false;
        }

        self.descrs_per_topic.contains_key(&channel.topic)
    }

    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        let message_descriptor = self.descrs_per_topic.get(&channel.topic)?;
        Some(Box::new(ProtobufMessageParser::new(
            num_rows,
            message_descriptor.clone(),
        )))
    }
}
