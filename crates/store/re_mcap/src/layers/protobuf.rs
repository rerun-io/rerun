use arrow::array::{
    ArrayBuilder, BinaryBuilder, BooleanBuilder, FixedSizeListBuilder, Float32Builder,
    Float64Builder, Int32Builder, Int64Builder, ListBuilder, StringBuilder, StructBuilder,
    UInt32Builder, UInt64Builder,
};
use arrow::datatypes::{DataType, Field, Fields};
use prost_reflect::{
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, ReflectMessage as _,
    Value,
};
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::reflection::ComponentDescriptorExt as _;

use crate::parsers::{MessageParser, ParserContext};
use crate::{Error, LayerIdentifier, MessageLayer};

struct ProtobufMessageParser {
    message_descriptor: MessageDescriptor,
    builder: FixedSizeListBuilder<StructBuilder>,
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

    #[error("type {0} is not supported yet")]
    UnsupportedType(&'static str),

    #[error("missing protobuf field {field}")]
    MissingField { field: u32 },
}

impl ProtobufMessageParser {
    fn new(num_rows: usize, message_descriptor: MessageDescriptor) -> Self {
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

        let struct_builder = struct_builder_from_message(&message_descriptor);
        let builder = FixedSizeListBuilder::with_capacity(struct_builder, 1, num_rows);

        Self {
            message_descriptor,
            builder,
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

        let struct_builder = self.builder.values();

        // Get the actual field descriptors from the schema to access their real field numbers.
        // This is critical for schemas with gaps in field numbering (e.g., fields 1, 2, 5, 8).
        let fields = self.message_descriptor.fields();

        for (field_builder, field_desc) in
            struct_builder.field_builders_mut().iter_mut().zip(fields)
        {
            // Use the actual field number from the schema, not index-based numbering.
            // Protobuf schemas can have gaps (e.g., fields 1, 2, 5, 8 after deprecating 3, 4).
            let protobuf_number = field_desc.number();

            if let Some(val) = dynamic_message.get_field_by_number(protobuf_number) {
                append_value(field_builder, &field_desc, val.as_ref())?;
                re_log::trace!(
                    field_name = %field_desc.full_name(), field_number = %protobuf_number, "Finished writing to builders",
                );
            } else {
                append_null_to_builder(field_builder)?;
            }
        }

        struct_builder.append(true);
        self.builder.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            message_descriptor,
            mut builder,
        } = *self;

        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            std::iter::once((
                ComponentDescriptor::partial("message")
                    .with_builtin_archetype(message_descriptor.full_name()),
                builder.finish().into(),
            ))
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

fn append_null_to_builder(builder: &mut dyn ArrayBuilder) -> Result<(), ProtobufError> {
    // Try to append null by downcasting to known builder types
    if let Some(b) = builder.as_any_mut().downcast_mut::<BooleanBuilder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<Int32Builder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<Int64Builder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<UInt32Builder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<UInt64Builder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<Float32Builder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<Float64Builder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<StringBuilder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<BinaryBuilder>() {
        b.append_null();
    } else if let Some(b) = builder.as_any_mut().downcast_mut::<StructBuilder>() {
        // `StructBuilder` mandates that all child arrays must share the same length as parent.
        // When appending null to parent, we must also append to children to maintain length.
        // Reference: https://arrow.apache.org/docs/format/Columnar.html#physical-memory-layout
        for child_builder in b.field_builders_mut() {
            append_null_to_builder(child_builder)?;
        }
        b.append_null();
    } else if let Some(b) = builder
        .as_any_mut()
        .downcast_mut::<ListBuilder<Box<dyn ArrayBuilder>>>()
    {
        b.append_null();
    } else {
        return Err(ProtobufError::UnsupportedType(
            "Unknown builder type for append_null",
        ));
    }
    Ok(())
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

            let struct_builder = downcast_err::<StructBuilder>(builder, val)?;
            let field_builders = struct_builder.field_builders_mut();

            // First field is "name" (String)
            downcast_err::<StringBuilder>(field_builders[0].as_mut(), val)?
                .append_value(value.name());

            // Second field is "value" (Int32)
            downcast_err::<Int32Builder>(field_builders[1].as_mut(), val)?.append_value(*x);

            struct_builder.append(true);
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
            // Create a struct with "name" (String) and "value" (Int32) fields.
            // We can't use `DictionaryArray` because `concat` does not re-key, and there
            // could be protobuf schema evolution with different enum values across chunks.
            let fields = Fields::from(vec![
                Field::new("name", DataType::Utf8, false),
                Field::new("value", DataType::Int32, false),
            ]);
            let field_builders: Vec<Box<dyn ArrayBuilder>> = vec![
                Box::new(StringBuilder::new()),
                Box::new(Int32Builder::new()),
            ];
            Box::new(StructBuilder::new(fields, field_builders))
        }
    };

    if descr.is_list() {
        return Box::new(ListBuilder::new(inner));
    }

    inner
}

fn arrow_field_from(descr: &FieldDescriptor) -> Field {
    let mut field = Field::new(descr.name(), datatype_from(descr), true);

    // Add extension metadata for enum types
    if matches!(descr.kind(), Kind::Enum(_)) {
        field = field.with_metadata(
            std::iter::once((
                "ARROW:extension:name".to_owned(),
                "rerun.datatypes.ProtobufEnum".to_owned(),
            ))
            .collect(),
        );
    }

    field
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
            // Struct with "name" (String) and "value" (Int32) fields.
            // See comment in arrow_builder_from_field for why we use a struct.
            let fields = Fields::from(vec![
                Field::new("name", DataType::Utf8, false),
                Field::new("value", DataType::Int32, false),
            ]);
            DataType::Struct(fields)
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
mod unit_tests {
    use arrow::array::{Array as _, ArrayBuilder, StringBuilder, StructBuilder};
    use arrow::datatypes::{DataType, Field, Fields};

    /// Verifies that `append_null_to_builder` properly handles `StructBuilder`
    /// by recursively appending nulls to child builders to maintain length consistency.
    #[test]
    fn struct_builder_null_append_bug() {
        // Create a StructBuilder with 2 child fields.
        let fields = Fields::from(vec![
            Field::new("a", DataType::Utf8, true),
            Field::new("b", DataType::Utf8, true),
        ]);
        let field_builders: Vec<Box<dyn ArrayBuilder>> = vec![
            Box::new(StringBuilder::new()),
            Box::new(StringBuilder::new()),
        ];
        let mut struct_builder = StructBuilder::new(fields, field_builders);

        // Test the fixed `append_null_to_builder` behavior:
        // It should recursively append nulls to children before appending to parent
        for _ in 0..10 {
            // Use our append_null_to_builder function which should handle this correctly
            super::append_null_to_builder(&mut struct_builder as &mut dyn ArrayBuilder)
                .expect("append_null_to_builder should succeed");
        }

        let array = struct_builder.finish();
        assert_eq!(array.len(), 10);
        assert_eq!(array.null_count(), 10); // All structs are null
    }
}

#[cfg(test)]
mod integration_tests {
    use std::io;

    use prost_reflect::prost::Message as _;
    use prost_reflect::prost_types::{
        DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto, FieldDescriptorProto,
        FileDescriptorProto, FileDescriptorSet, field_descriptor_proto,
    };
    use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
    use re_chunk::Chunk;

    use crate::LayerRegistry;
    use crate::layers::McapProtobufLayer;

    fn create_pool_with_person() -> MessageDescriptor {
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

        let pool =
            DescriptorPool::decode(encoded.as_slice()).expect("failed to decode descriptor pool");
        pool.get_message_by_name("com.example.Person")
            .expect("missing message descriptor")
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
            let person_message = create_pool_with_person();

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

    /// This test verifies that we are resilient to decode failures. When messages fail to decode,
    /// they should be logged and skipped without causing length mismatches.
    #[test]
    fn decode_failure_resilience() {
        use prost_reflect::prost::Message as _;

        let (summary, buffer) = {
            let person_message = create_pool_with_person();

            let buffer = Vec::new();
            let cursor = io::Cursor::new(buffer);
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let channel_id = add_schema_and_channel(&mut writer, &person_message, "test_topic")
                .expect("failed to add schema and channel");

            // Write a mix of valid messages and completely invalid protobuf data
            for i in 0..10 {
                let bytes = if i % 2 == 0 {
                    // Write completely invalid protobuf data (random bytes that will fail to decode)
                    vec![0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xAA, 0xBB]
                } else {
                    // Write a valid message
                    let msg = DynamicMessage::parse_text_format(
                        person_message.clone(),
                        &format!("name: \"Person{i}\" id: {i}"),
                    )
                    .expect("failed to parse text format");
                    msg.encode_to_vec()
                };

                // Write the (valid or invalid) message directly
                writer
                    .write_to_known_channel(
                        &mcap::records::MessageHeader {
                            channel_id,
                            sequence: 0,
                            log_time: 100 + i,
                            publish_time: 100 + i,
                        },
                        &bytes,
                    )
                    .expect("failed to write message");
            }

            let summary = writer.finish().expect("finishing writer failed");
            (summary, writer.into_inner().into_inner())
        };

        // Verify that decode failures don't cause panics - only valid messages should be returned
        let chunks = run_layer(&summary, buffer.as_slice());
        assert_eq!(chunks.len(), 1);
        // We wrote 10 messages (5 valid, 5 invalid), so we should get 5 rows
        assert_eq!(chunks[0].num_rows(), 5);
    }

    /// Test with field number gaps in the protobuf schema.
    #[test]
    fn field_number_gaps() {
        use prost_reflect::prost::Message as _;
        use prost_reflect::prost_types::field_descriptor_proto;

        // Create a protobuf schema with gaps in field numbers
        let pool = prost_reflect::DescriptorPool::decode(
            prost_reflect::prost_types::FileDescriptorSet {
                file: vec![prost_reflect::prost_types::FileDescriptorProto {
                    name: Some("test.proto".to_owned()),
                    package: Some("test".to_owned()),
                    message_type: vec![prost_reflect::prost_types::DescriptorProto {
                        name: Some("MessageWithGaps".to_owned()),
                        field: vec![
                            prost_reflect::prost_types::FieldDescriptorProto {
                                name: Some("name".to_owned()),
                                number: Some(1),
                                r#type: Some(field_descriptor_proto::Type::String as i32),
                                ..Default::default()
                            },
                            prost_reflect::prost_types::FieldDescriptorProto {
                                name: Some("id".to_owned()),
                                number: Some(2),
                                r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                                ..Default::default()
                            },
                            prost_reflect::prost_types::FieldDescriptorProto {
                                name: Some("description".to_owned()),
                                number: Some(5), // Gap: fields 3, 4 are just missing
                                r#type: Some(field_descriptor_proto::Type::String as i32),
                                ..Default::default()
                            },
                            prost_reflect::prost_types::FieldDescriptorProto {
                                name: Some("count".to_owned()),
                                number: Some(8), // Gap: fields 6, 7 are reserved
                                r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                                ..Default::default()
                            },
                        ],
                        reserved_range: vec![
                            prost_reflect::prost_types::descriptor_proto::ReservedRange {
                                start: Some(6),
                                end: Some(8), // end is exclusive, so this reserves 6 and 7
                            },
                        ],
                        ..Default::default()
                    }],
                    syntax: Some("proto3".to_owned()),
                    ..Default::default()
                }],
            }
            .encode_to_vec()
            .as_slice(),
        )
        .expect("failed to create descriptor pool");

        let message_descriptor = pool
            .get_message_by_name("test.MessageWithGaps")
            .expect("missing message descriptor");

        println!("Message descriptor fields:");
        for field in message_descriptor.fields() {
            println!("  Field #{}: {}", field.number(), field.name());
        }

        let (summary, buffer) = {
            let buffer = Vec::new();
            let cursor = io::Cursor::new(buffer);
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let channel_id = add_schema_and_channel(&mut writer, &message_descriptor, "test_topic")
                .expect("failed to add schema and channel");

            // Write 10 test messages
            for i in 0..10 {
                let msg = DynamicMessage::parse_text_format(
                    message_descriptor.clone(),
                    &format!(
                        "name: \"Name{}\" id: {} description: \"Desc{}\" count: {}",
                        i,
                        i,
                        i,
                        i * 10
                    ),
                )
                .expect("failed to parse text format");

                write_message(&mut writer, channel_id, &msg, 100 + i)
                    .expect("failed to write message");
            }

            let summary = writer.finish().expect("finishing writer failed");
            (summary, writer.into_inner().into_inner())
        };

        // With the fix, field number gaps should be handled correctly:
        // We create 4 builders (one per field with numbers 1, 2, 5, 8)
        // and correctly map them to their actual protobuf field numbers.
        // Result: All fields should be read correctly with their values intact!
        let chunks = run_layer(&summary, buffer.as_slice());

        // Verify we got all messages
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].num_rows(), 10);

        // Verify the data contains actual values, not nulls
        // (Before the fix, description and count would be null due to field mapping bug)
        let chunk_str = format!("{:-120}", &chunks[0]);

        // Check that we have actual description values (not all nulls)
        assert!(
            chunk_str.contains("Desc"),
            "Field 'description' (field #5) should have values, not nulls"
        );

        // Check that we have actual count values (not all nulls)
        assert!(
            chunk_str.contains("count:"),
            "Field 'count' (field #8) should have values, not nulls"
        );
    }

    /// In `proto3`, all fields are optional, so we test various combinations of missing fields.
    #[test]
    fn missing_optional_fields() {
        let (summary, buffer) = {
            let person_message = create_pool_with_person();

            let buffer = Vec::new();
            let cursor = io::Cursor::new(buffer);
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let channel_id = add_schema_and_channel(&mut writer, &person_message, "test_topic")
                .expect("failed to add schema and channel");

            // Message 1: has all fields
            let dynamic_message_1 = DynamicMessage::parse_text_format(
                person_message.clone(),
                "name: \"Alice\" id: 123 status: 1",
            )
            .expect("failed to parse text format");

            // Message 2: has only name (id and status missing)
            // This tests the bug - struct with missing optional fields
            let dynamic_message_2 =
                DynamicMessage::parse_text_format(person_message.clone(), "name: \"Bob\"")
                    .expect("failed to parse text format");

            // Message 3: has only id (name and status missing)
            let dynamic_message_3 =
                DynamicMessage::parse_text_format(person_message.clone(), "id: 456")
                    .expect("failed to parse text format");

            // Message 4: has only status (name and id missing)
            let dynamic_message_4 =
                DynamicMessage::parse_text_format(person_message.clone(), "status: 2")
                    .expect("failed to parse text format");

            // Message 5: empty message (all fields missing)
            // This is the most extreme case - may trigger the crash
            let dynamic_message_5 = DynamicMessage::parse_text_format(person_message.clone(), "")
                .expect("failed to parse text format");

            write_message(&mut writer, channel_id, &dynamic_message_1, 42)
                .expect("failed to write message");
            write_message(&mut writer, channel_id, &dynamic_message_2, 43)
                .expect("failed to write message");
            write_message(&mut writer, channel_id, &dynamic_message_3, 44)
                .expect("failed to write message");
            write_message(&mut writer, channel_id, &dynamic_message_4, 45)
                .expect("failed to write message");
            write_message(&mut writer, channel_id, &dynamic_message_5, 46)
                .expect("failed to write message");

            let summary = writer.finish().expect("finishing writer failed");

            (summary, writer.into_inner().into_inner())
        };

        let chunks = run_layer(&summary, buffer.as_slice());
        assert_eq!(chunks.len(), 1);

        insta::assert_snapshot!(
            "missing_optional_fields_proto3",
            format!("{:-240}", &chunks[0])
        );
    }
}
