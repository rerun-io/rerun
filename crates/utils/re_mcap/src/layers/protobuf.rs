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
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, ReflectMessage as _,
    Value,
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
enum ProtobufError {
    #[error("invalid message on channel {channel} for schema {schema}: {source}")]
    InvalidMessage {
        schema: String,
        channel: String,
        source: prost_reflect::prost::DecodeError,
    },

    #[error("expected type {expected}, but found value {actual}")]
    UnexpectedValue {
        expected: &'static str,
        actual: Value,
    },

    #[error("expected type {expected}, but found kind {actual:?}")]
    UnexpectedType {
        expected: &'static str,
        actual: prost_reflect::Kind,
    },

    #[error("unknown enum number {0}")]
    UnknownEnumNumber(i32),

    #[error("unknown field name {0}")]
    UnknownFieldName(String),

    #[error("type {0} is not supported yet")]
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
                let field = dynamic_message
                    .descriptor()
                    .get_field_by_name(field)
                    .ok_or_else(|| ProtobufError::UnknownFieldName(field.to_owned()))?;
                append_value(builder.values(), &field, val.as_ref())?;
                builder.append(true);
                re_log::trace!("Field {}: Finished writing to builders", field.full_name());
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
            expected: type_name.strip_suffix("Builder").unwrap_or(type_name),
            actual: val.clone(),
        }
    })
}

fn append_value(
    builder: &mut dyn ArrayBuilder,
    field: &FieldDescriptor,
    val: &Value,
) -> Result<(), ProtobufError> {
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
                let field = dynamic_message
                    .descriptor()
                    .get_field(protobuf_number)
                    .ok_or_else(|| ProtobufError::MissingField {
                        field: protobuf_number,
                    })?;
                append_value(field_builder, &field, val.as_ref())?;
            }
            struct_builder.append(true);
        }
        Value::List(vec) => {
            re_log::trace!("Append called on a list with {} elements: {val}", vec.len(),);
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;

            for val in vec {
                // All of these values still belong to the same field,
                // which is why we forward the descriptor.
                append_value(list_builder.values(), field, val)?;
            }
            list_builder.append(true);
            re_log::trace!("Finished append on list with elements {val}");
        }
        Value::Map(_hash_map) => {
            // We should not encounter hash maps in protobufs.
            return Err(ProtobufError::UnsupportedType("HashMap"));
        }
        Value::EnumNumber(x) => {
            let kind = field.kind();
            let enum_descriptor = kind
                .as_enum()
                .ok_or_else(|| ProtobufError::UnexpectedType {
                    expected: "enum",
                    actual: kind.clone(),
                })?;
            let value = enum_descriptor
                .get_value(*x)
                .ok_or_else(|| ProtobufError::UnknownEnumNumber(*x))?;
            downcast_err::<StringBuilder>(builder, val)?.append_value(value.name());
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
            // TODO(grtlr): It would be great to improve our `enum` support. Using `Utf8`
            // means a lot of excess memory / storage usage. Ideally we would use something
            // like `StringDictionary`, but it's not clear right now how this works with
            // `dyn ArrayBuilder` and sharing entries across lists.
            Box::new(StringBuilder::new())
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
            // TODO(grtlr): Explanation see above.
            DataType::Utf8
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

#[cfg(test)]
mod test {
    use std::io;

    use prost_reflect::{
        DescriptorPool, DynamicMessage, MessageDescriptor,
        prost::Message as _,
        prost_types::{
            DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto, FieldDescriptorProto,
            FileDescriptorProto, FileDescriptorSet, field_descriptor_proto,
        },
    };
    use re_chunk::Chunk;

    use crate::{LayerRegistry, layers::McapProtobufLayer};

    fn create_pool() -> DescriptorPool {
        let status = EnumDescriptorProto {
            name: Some("Status".into()),
            value: vec![
                EnumValueDescriptorProto {
                    name: Some("UNKNOWN".into()),
                    number: Some(0),
                    options: None,
                },
                EnumValueDescriptorProto {
                    name: Some("ACTIVE".into()),
                    number: Some(1),
                    options: None,
                },
                EnumValueDescriptorProto {
                    name: Some("INACTIVE".into()),
                    number: Some(2),
                    options: None,
                },
            ],
            options: None,
            reserved_range: vec![],
            reserved_name: vec![],
        };

        // Create a simple message descriptor
        let person_message = DescriptorProto {
            name: Some("Person".into()),
            field: vec![
                FieldDescriptorProto {
                    name: Some("name".into()),
                    number: Some(1),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("id".into()),
                    number: Some(2),
                    r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("status".into()),
                    number: Some(3),
                    r#type: Some(field_descriptor_proto::Type::Enum as i32),
                    type_name: Some("Status".into()),
                    ..Default::default()
                },
            ],
            enum_type: vec![status],
            ..Default::default()
        };

        let file_proto = FileDescriptorProto {
            name: Some("person.proto".into()),
            package: Some("com.example".into()),
            message_type: vec![person_message],
            syntax: Some("proto3".into()),
            ..Default::default()
        };

        let file_descriptor_set = FileDescriptorSet {
            file: vec![file_proto],
        };

        let encoded = file_descriptor_set.encode_to_vec();

        DescriptorPool::decode(encoded.as_slice()).expect("failed to decode descriptor pool")
    }

    /// Returns a channel id.
    fn add_schema_and_channel<W: io::Write + io::Seek>(
        writer: &mut mcap::Writer<W>,
        message_descriptor: &MessageDescriptor,
        topic: &str,
    ) -> mcap::McapResult<u16> {
        let data = message_descriptor.parent_pool().encode_to_vec();

        let schema_id =
            writer.add_schema(message_descriptor.full_name(), "protobuf", data.as_slice())?;

        let channel_id = writer.add_channel(schema_id, topic, "protobuf", &Default::default())?;
        Ok(channel_id)
    }

    fn write_message<W: io::Write + io::Seek>(
        writer: &mut mcap::Writer<W>,
        channel_id: u16,
        message: &DynamicMessage,
        timestamp: u64, // nanoseconds since epoch
    ) -> mcap::McapResult<()> {
        // Encode the dynamic message to protobuf bytes
        let data = message.encode_to_vec();

        let header = mcap::records::MessageHeader {
            channel_id,
            sequence: 0,
            log_time: timestamp,
            publish_time: timestamp,
        };

        writer.write_to_known_channel(&header, data.as_slice())?;

        Ok(())
    }

    fn run_layer(summary: &mcap::Summary, buffer: &[u8]) -> Vec<Chunk> {
        let mut chunks = Vec::new();

        let mut send_chunk = |chunk| {
            chunks.push(chunk);
        };

        let registry = LayerRegistry::empty().register_message_layer::<McapProtobufLayer>();
        registry
            .plan(summary)
            .expect("failed to plan")
            .run(buffer, summary, &mut send_chunk)
            .expect("failed to run layer");

        chunks
    }

    #[test]
    fn two_simple_rows() {
        // Writing to the MCAP buffer.
        let (summary, buffer) = {
            let pool = create_pool();
            let person_message = pool
                .get_message_by_name("com.example.Person")
                .expect("missing message descriptor");

            let buffer = Vec::new();
            let cursor = io::Cursor::new(buffer);
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let channel_id = add_schema_and_channel(&mut writer, &person_message, "test_topic")
                .expect("failed to add schema and channel");

            let dynamic_message_1 =
                DynamicMessage::parse_text_format(person_message.clone(), "name: \"Bob\"status:2")
                    .expect("failed to parse text format");

            let dynamic_message_2 =
                DynamicMessage::parse_text_format(person_message.clone(), "name: \"Alice\"id:123")
                    .expect("failed to parse text format");

            write_message(&mut writer, channel_id, &dynamic_message_1, 42)
                .expect("failed to write message");
            write_message(&mut writer, channel_id, &dynamic_message_2, 43)
                .expect("failed to write message");

            let summary = writer.finish().expect("finishing writer failed");

            (summary, writer.into_inner().into_inner())
        };
        assert_eq!(
            summary.chunk_indexes.len(),
            1,
            "there should be only one chunk"
        );

        let chunks = run_layer(&summary, buffer.as_slice());
        assert_eq!(chunks.len(), 1);

        insta::assert_snapshot!("two_simple_rows", format!("{:-240}", &chunks[0]));
    }
}
