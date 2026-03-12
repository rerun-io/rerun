use std::sync::Arc;

use arrow::array::{
    ArrayBuilder, BinaryBuilder, BooleanBuilder, FixedSizeListBuilder, Float32Builder,
    Float64Builder, Int32Builder, Int64Builder, ListBuilder, MapBuilder, MapFieldNames,
    StringBuilder, StructBuilder, UInt32Builder, UInt64Builder,
};
use arrow::datatypes::{DataType, Field, Fields};
use prost_reflect::{
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MapKey, MessageDescriptor,
    OneofDescriptor, ReflectMessage as _, Value,
};
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::reflection::ComponentDescriptorExt as _;

use crate::parsers::{MessageParser, ParserContext};
use crate::{DecoderIdentifier, Error, MessageDecoder};

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

    #[error("appending to array failed: {0}")]
    AppendFailed(#[from] arrow::error::ArrowError),

    #[error("unknown enum number {0}")]
    UnknownEnumNumber(i32),

    #[error("type {0} is not supported yet")]
    UnsupportedType(&'static str),
}

impl ProtobufMessageParser {
    fn new(num_rows: usize, message_descriptor: MessageDescriptor) -> Self {
        let struct_builder = struct_builder_from_message(&message_descriptor);
        let builder = FixedSizeListBuilder::with_capacity(struct_builder, 1, num_rows);

        Self {
            message_descriptor,
            builder,
        }
    }
}

/// Helper function to append fields from a protobuf message to a struct builder.
/// This handles both proto3 optional field semantics and regular proto3 fields:
/// - For proto3 optional fields (presence tracking): append null for unset fields
/// - For regular proto3 fields (no presence tracking): append default values for unset fields
fn append_message_fields(
    dynamic_message: &DynamicMessage,
    struct_builder: &mut StructBuilder,
) -> Result<(), ProtobufError> {
    // Get the actual field descriptors from the schema to access their real field numbers.
    // This is critical for schemas with gaps in field numbering (e.g., fields 1, 2, 5, 8).
    let descriptor = dynamic_message.descriptor();

    // Create a map of field number -> value for fields that were actually set.
    // In proto3, scalar fields don't have presence tracking unless marked `optional`,
    // so we use fields() which only returns fields that were explicitly set.
    let set_fields: ahash::HashMap<u32, &prost_reflect::Value> = dynamic_message
        .fields()
        .map(|(field_desc, value)| (field_desc.number(), value))
        .collect();

    let grouped = grouped_fields(&descriptor);

    for (field_builder, grouped_field) in struct_builder
        .field_builders_mut()
        .iter_mut()
        .zip(grouped.iter())
    {
        match grouped_field {
            GroupedField::Regular(field_desc) => {
                // Use the actual field number from the schema, not index-based numbering.
                // Protobuf schemas can have gaps (e.g., fields 1, 2, 5, 8 after deprecating 3, 4).
                let protobuf_number = field_desc.number();

                if let Some(val) = set_fields.get(&protobuf_number) {
                    append_value(field_builder, field_desc, val)?;
                } else {
                    // For proto3 optional fields, maps, and lists: append null for unset fields.
                    // For regular proto3 fields: append default values.
                    if field_desc.supports_presence() || field_desc.is_map() || field_desc.is_list()
                    {
                        append_null_to_builder(field_builder)?;
                    } else {
                        // Use the default value for this field type.
                        let default_value = field_desc.default_value();
                        append_value(field_builder, field_desc, &default_value)?;
                    }
                }
            }
            GroupedField::OneOf(oneof) => {
                let oneof_builder = field_builder
                    .as_any_mut()
                    .downcast_mut::<StructBuilder>()
                    .expect("oneof wrapper builder should be a StructBuilder");

                // Find which variant (if any) is set in this message.
                let mut any_set = false;
                for (variant_builder, variant_field) in oneof_builder
                    .field_builders_mut()
                    .iter_mut()
                    .zip(oneof.fields())
                {
                    if let Some(val) = set_fields.get(&variant_field.number()) {
                        append_value(variant_builder, &variant_field, val)?;
                        any_set = true;
                    } else {
                        append_null_to_builder(variant_builder)?;
                    }
                }

                // Append the wrapper struct: true if any variant is set, false (null) otherwise.
                oneof_builder.append(any_set);
            }
        }
    }

    struct_builder.append(true);
    Ok(())
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
        append_message_fields(&dynamic_message, struct_builder)?;
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
    // Try to append null by downcasting to known builder types.
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
    } else if let Some(b) = builder
        .as_any_mut()
        .downcast_mut::<MapBuilder<Box<dyn ArrayBuilder>, Box<dyn ArrayBuilder>>>()
    {
        b.append(false)?;
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
            let struct_builder = downcast_err::<StructBuilder>(builder, val)?;
            append_message_fields(dynamic_message, struct_builder)?;
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
        Value::Map(hash_map) => {
            let Kind::Message(entry_msg) = field.kind() else {
                return Err(ProtobufError::UnexpectedType {
                    expected: "message (map entry)",
                    actual: field.kind(),
                });
            };
            let key_field = entry_msg.map_entry_key_field();
            let value_field = entry_msg.map_entry_value_field();

            let map_builder = downcast_err::<
                MapBuilder<Box<dyn ArrayBuilder>, Box<dyn ArrayBuilder>>,
            >(builder, val)?;

            let mut sorted_entries: Vec<_> = hash_map.iter().collect();
            sorted_entries.sort_by_key(|(k, _)| (*k).clone());
            for (map_key, map_value) in sorted_entries {
                let key_value = map_key_to_value(map_key);
                append_value(map_builder.keys(), &key_field, &key_value)?;
                append_value(map_builder.values(), &value_field, map_value)?;
            }
            map_builder
                .append(true)
                .map_err(|_err| ProtobufError::UnsupportedType("MapBuilder append failed"))?;
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

            // First field is "name" (String).
            downcast_err::<StringBuilder>(field_builders[0].as_mut(), val)?
                .append_value(value.name());

            // Second field is "value" (Int32).
            downcast_err::<Int32Builder>(field_builders[1].as_mut(), val)?.append_value(*x);

            struct_builder.append(true);
        }
    }

    Ok(())
}

/// Converts a [`MapKey`] to a [`Value`] so it can be appended via `append_value`.
fn map_key_to_value(key: &MapKey) -> Value {
    match key {
        MapKey::Bool(b) => Value::Bool(*b),
        MapKey::I32(i) => Value::I32(*i),
        MapKey::I64(i) => Value::I64(*i),
        MapKey::U32(u) => Value::U32(*u),
        MapKey::U64(u) => Value::U64(*u),
        MapKey::String(s) => Value::String(s.clone()),
    }
}

enum GroupedField {
    Regular(FieldDescriptor),
    OneOf(OneofDescriptor),
}

/// Iterates the fields of a message descriptor, collapsing real oneof variants
/// (those with more than one field) into a single [`GroupedField::OneOf`] entry
/// at the position of the first variant.
fn grouped_fields(descriptor: &MessageDescriptor) -> Vec<GroupedField> {
    let mut result = Vec::new();
    let mut seen = ahash::HashSet::default();
    for field in descriptor.fields() {
        // Synthetic oneofs (single-field, proto3 optional) remain as [`GroupedField::Regular`].
        if let Some(oneof) = field.containing_oneof()
            && oneof.fields().len() > 1
        {
            if seen.insert(oneof.full_name().to_owned()) {
                result.push(GroupedField::OneOf(oneof));
            }
            continue;
        }
        result.push(GroupedField::Regular(field));
    }
    result
}

/// Creates Arrow [`Fields`] from a message descriptor, wrapping real oneof
/// variants in a single struct field named after the oneof with
/// `ProtobufOneOf` metadata on the wrapper.
fn fields_from_message(descriptor: &MessageDescriptor) -> Fields {
    grouped_fields(descriptor)
        .into_iter()
        .map(|gf| match gf {
            GroupedField::Regular(f) => arrow_field_from(&f),
            GroupedField::OneOf(oneof) => {
                let inner: Fields = oneof.fields().map(|f| arrow_field_from(&f)).collect();
                Field::new(oneof.name(), DataType::Struct(inner), true).with_metadata(
                    std::iter::once((
                        "ARROW:extension:name".to_owned(),
                        "rerun.datatypes.ProtobufOneOf".to_owned(),
                    ))
                    .collect(),
                )
            }
        })
        .collect()
}

fn struct_builder_from_message(message_descriptor: &MessageDescriptor) -> StructBuilder {
    let fields = fields_from_message(message_descriptor);
    let field_builders: Vec<Box<dyn ArrayBuilder>> = grouped_fields(message_descriptor)
        .into_iter()
        .map(|gf| -> Box<dyn ArrayBuilder> {
            match gf {
                GroupedField::Regular(f) => arrow_builder_from_field(&f),
                GroupedField::OneOf(oneof) => {
                    let inner_fields: Fields =
                        oneof.fields().map(|f| arrow_field_from(&f)).collect();
                    let inner_builders: Vec<Box<dyn ArrayBuilder>> = oneof
                        .fields()
                        .map(|f| arrow_builder_from_field(&f))
                        .collect();
                    Box::new(StructBuilder::new(inner_fields, inner_builders))
                }
            }
        })
        .collect();

    re_log::debug_assert_eq!(fields.len(), field_builders.len());

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
        Kind::Message(message_descriptor) if descr.is_map() => {
            let key_field = message_descriptor.map_entry_key_field();
            let val_field = message_descriptor.map_entry_value_field();
            let field_names = MapFieldNames {
                entry: "entries".to_owned(),
                key: key_field.name().to_owned(),
                value: val_field.name().to_owned(),
            };
            let key_builder = arrow_builder_from_field(&key_field);
            let val_builder = arrow_builder_from_field(&val_field);
            return Box::new(MapBuilder::new(Some(field_names), key_builder, val_builder));
        }
        Kind::Message(message_descriptor) => {
            Box::new(struct_builder_from_message(&message_descriptor)) as Box<dyn ArrayBuilder>
        }
        Kind::Enum(_) => {
            // Create a struct with "name" (String) and "value" (Int32) fields.
            // We can't use `DictionaryArray` because `concat` does not re-key, and there
            // could be protobuf schema evolution with different enum values across chunks.
            // The child fields are nullable to support null enum values when the parent field is missing.
            let fields = Fields::from(vec![
                Field::new("name", DataType::Utf8, true),
                Field::new("value", DataType::Int32, true),
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

    // Add extension metadata for enum types.
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
        Kind::Message(message_descriptor) if descr.is_map() => {
            let proto_key_field = message_descriptor.map_entry_key_field();
            let proto_val_field = message_descriptor.map_entry_value_field();
            let key_field = Field::new(
                proto_key_field.name(),
                datatype_from(&proto_key_field),
                false,
            );
            let val_field = Field::new(
                proto_val_field.name(),
                datatype_from(&proto_val_field),
                true,
            );
            let entry_field = Field::new(
                "entries",
                DataType::Struct(Fields::from(vec![key_field, val_field])),
                false,
            );
            // TODO(grtlr): We actually store the data sorted, but `MapBuilder` does not allow that.
            DataType::Map(Arc::new(entry_field), false)
        }
        Kind::Message(message_descriptor) => {
            DataType::Struct(fields_from_message(&message_descriptor))
        }
        Kind::Enum(_) => {
            // Struct with "name" (String) and "value" (Int32) fields.
            // See comment in arrow_builder_from_field for why we use a struct.
            // The child fields are nullable to support null enum values when the parent field is missing.
            let fields = Fields::from(vec![
                Field::new("name", DataType::Utf8, true),
                Field::new("value", DataType::Int32, true),
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
/// Applying this decoder will result in a direct Arrow representation of the fields.
/// This is useful for querying certain fields from an MCAP file, but wont result
/// in semantic types that can be picked up by the Rerun viewer.
#[derive(Debug, Default)]
pub struct McapProtobufDecoder {
    descrs_per_topic: ahash::HashMap<String, MessageDescriptor>,
}

impl MessageDecoder for McapProtobufDecoder {
    fn identifier() -> DecoderIdentifier {
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
            re_log::debug_assert!(found.is_none());
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
    fn test_struct_builder_null_append() {
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

        // It should recursively append nulls to children before appending to parent.
        for _ in 0..10 {
            // Use our append_null_to_builder function which should handle this correctly.
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
        FileDescriptorProto, FileDescriptorSet, MessageOptions, OneofDescriptorProto,
        field_descriptor_proto,
    };
    use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
    use re_chunk::Chunk;

    use re_log_types::TimeType;

    use crate::DecoderRegistry;
    use crate::decoders::McapProtobufDecoder;

    fn format_chunk(chunk: &Chunk) -> String {
        let batch = chunk.to_record_batch().expect("failed to convert chunk");
        re_arrow_util::RecordBatchFormatOpts {
            width: Some(240),
            max_cell_content_width: usize::MAX,
            redact_non_deterministic: true,
            ..Default::default()
        }
        .format(&batch)
        .to_string()
    }

    /// Helper to mark fields as proto3 optional with proper synthetic oneof declarations.
    ///
    /// Returns the modified fields and the synthetic `OneofDescriptorProto` entries that
    /// must be added to the parent message's `oneof_decl`. The `oneof_index_offset` is
    /// used when the message already has real oneofs declared before these synthetic ones.
    #[expect(
        clippy::cast_possible_wrap,
        reason = "test helper with small field counts"
    )]
    fn make_fields_optional(
        fields: Vec<FieldDescriptorProto>,
        oneof_index_offset: i32,
    ) -> (Vec<FieldDescriptorProto>, Vec<OneofDescriptorProto>) {
        let mut oneof_decls = Vec::new();
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(i, mut field)| {
                field.label = Some(field_descriptor_proto::Label::Optional as i32);
                field.proto3_optional = Some(true);
                field.oneof_index = Some(oneof_index_offset + i as i32);
                oneof_decls.push(OneofDescriptorProto {
                    name: Some(format!("_{}", field.name.as_ref().unwrap())),
                    ..Default::default()
                });
                field
            })
            .collect();
        (fields, oneof_decls)
    }

    /// Helper to create a [`MessageDescriptor`] from a list of [`DescriptorProto`].
    /// Takes a list of message descriptors and the fully qualified name of the message to extract.
    fn create_message_descriptor(
        message_protos: Vec<DescriptorProto>,
        full_name: &str,
    ) -> MessageDescriptor {
        let file_descriptor = FileDescriptorProto {
            name: Some("test.proto".into()),
            package: Some("com.example".into()),
            message_type: message_protos,
            syntax: Some("proto3".into()),
            ..Default::default()
        };

        let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet {
            file: vec![file_descriptor],
        })
        .expect("failed to create descriptor pool");

        pool.get_message_by_name(full_name)
            .expect("failed to get message descriptor")
    }

    /// Creates a `Person` message descriptor equivalent to:
    ///
    /// ```protobuf
    /// message Person {
    ///   enum Status {
    ///     UNKNOWN = 0;
    ///     ACTIVE = 1;
    ///     INACTIVE = 2;
    ///   }
    ///
    ///   message Address {
    ///     optional string street = 1;
    ///     optional string city = 2;
    ///   }
    ///
    ///   optional string name = 1;
    ///   reserved 2 to 4;
    ///   optional int32 id = 5;
    ///   optional Status status = 8;
    ///   optional Address address = 9;
    ///   map<string, string> tags = 10;
    /// }
    /// ```
    ///
    /// If `use_proto3_optional` is `false`, the `optional` keywords are removed and
    /// fields will not have presence tracking (unset fields show default values).
    fn create_person_descriptor(use_proto3_optional: bool) -> (&'static str, DescriptorProto) {
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

        // Create a nested Address message.
        let address_fields = vec![
            FieldDescriptorProto {
                name: Some("street".into()),
                number: Some(1),
                r#type: Some(field_descriptor_proto::Type::String as i32),
                ..Default::default()
            },
            FieldDescriptorProto {
                name: Some("city".into()),
                number: Some(2),
                r#type: Some(field_descriptor_proto::Type::String as i32),
                ..Default::default()
            },
        ];

        let (address_fields, address_oneof_decls) = if use_proto3_optional {
            make_fields_optional(address_fields, 0)
        } else {
            (address_fields, vec![])
        };

        let address_message = DescriptorProto {
            name: Some("Address".into()),
            field: address_fields,
            oneof_decl: address_oneof_decls,
            ..Default::default()
        };

        // Create field descriptors with gaps in field numbering to test handling of schemas
        // with non-contiguous field numbers and reserved ranges between actual fields.
        // Field 1: name, Reserved: 2-4, Field 5: id, Field 8: status, Field 9: address.
        let fields = vec![
            FieldDescriptorProto {
                name: Some("name".into()),
                number: Some(1),
                r#type: Some(field_descriptor_proto::Type::String as i32),
                ..Default::default()
            },
            FieldDescriptorProto {
                name: Some("id".into()),
                number: Some(5), // Gap: fields 2-4 are reserved.
                r#type: Some(field_descriptor_proto::Type::Int32 as i32),
                ..Default::default()
            },
            FieldDescriptorProto {
                name: Some("status".into()),
                number: Some(8), // Gap: fields 6-7 are missing.
                r#type: Some(field_descriptor_proto::Type::Enum as i32),
                type_name: Some("Status".into()),
                ..Default::default()
            },
            FieldDescriptorProto {
                name: Some("address".into()),
                number: Some(9),
                r#type: Some(field_descriptor_proto::Type::Message as i32),
                type_name: Some("Address".into()),
                ..Default::default()
            },
        ];

        let (mut fields, person_oneof_decls) = if use_proto3_optional {
            make_fields_optional(fields, 0)
        } else {
            (fields, vec![])
        };

        // Map field added *after* `make_fields_optional` so it is not wrapped in a synthetic oneof.
        let tags_entry = DescriptorProto {
            name: Some("TagsEntry".into()),
            field: vec![
                FieldDescriptorProto {
                    name: Some("key".into()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("value".into()),
                    number: Some(2),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    ..Default::default()
                },
            ],
            options: Some(MessageOptions {
                map_entry: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        fields.push(FieldDescriptorProto {
            name: Some("tags".into()),
            number: Some(10),
            label: Some(field_descriptor_proto::Label::Repeated as i32),
            r#type: Some(field_descriptor_proto::Type::Message as i32),
            type_name: Some("TagsEntry".into()),
            ..Default::default()
        });

        // Create a message descriptor with reserved field numbers (2, 3, 4) between actual fields.
        let person_proto = DescriptorProto {
            name: Some("Person".into()),
            field: fields,
            nested_type: vec![address_message, tags_entry],
            enum_type: vec![status],
            oneof_decl: person_oneof_decls,
            reserved_range: vec![
                prost_reflect::prost_types::descriptor_proto::ReservedRange {
                    start: Some(2),
                    end: Some(5), // end is exclusive, so this reserves 2, 3, 4.
                },
            ],
            ..Default::default()
        };

        ("com.example.Person", person_proto)
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
        // Encode the dynamic message to protobuf bytes.
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

    fn run_decoder(summary: &mcap::Summary, buffer: &[u8]) -> Vec<Chunk> {
        let mut chunks = Vec::new();

        let mut send_chunk = |chunk| {
            chunks.push(chunk);
        };

        let registry = DecoderRegistry::empty().register_message_decoder::<McapProtobufDecoder>();
        registry
            .plan(summary)
            .expect("failed to plan")
            .run(buffer, summary, TimeType::TimestampNs, &mut send_chunk)
            .expect("failed to run decoder");

        chunks
    }

    /// Helper to create test messages with various field combinations.
    fn create_test_messages(person_message: &MessageDescriptor) -> Vec<DynamicMessage> {
        use prost_reflect::{MapKey, Value};

        vec![
            // Message 1: has all fields including nested address and tags.
            {
                let mut msg = DynamicMessage::parse_text_format(
                    person_message.clone(),
                    "name: \"Alice\" id: 123 status: 1 address: { street: \"Main St\" city: \"NYC\" }",
                )
                .expect("failed to parse text format");
                let mut tags = std::collections::HashMap::new();
                tags.insert(MapKey::String("role".into()), Value::String("admin".into()));
                tags.insert(MapKey::String("org".into()), Value::String("rerun".into()));
                msg.set_field_by_name("tags", Value::Map(tags));
                msg
            },
            // Message 2: has name and status, with partial address (only street).
            DynamicMessage::parse_text_format(
                person_message.clone(),
                "name: \"Bob\" status: 2 address: { street: \"Oak Ave\" }",
            )
            .expect("failed to parse text format"),
            // Message 3: has name and id, no address.
            DynamicMessage::parse_text_format(person_message.clone(), "name: \"Charlie\" id: 456")
                .expect("failed to parse text format"),
            // Message 4: has only name, nested address, and tags.
            {
                let mut msg = DynamicMessage::parse_text_format(
                    person_message.clone(),
                    "name: \"Dave\" address: { city: \"LA\" }",
                )
                .expect("failed to parse text format");
                let mut tags = std::collections::HashMap::new();
                tags.insert(MapKey::String("role".into()), Value::String("admin".into()));
                msg.set_field_by_name("tags", Value::Map(tags));
                msg
            },
            // Message 5: has only id (name, status, and address missing).
            {
                let mut msg = DynamicMessage::new(person_message.clone());
                msg.set_field_by_name("id", Value::I32(789));
                msg
            },
            // Message 6: has only status (name, id, and address missing).
            {
                let mut msg = DynamicMessage::new(person_message.clone());
                msg.set_field_by_name("status", Value::EnumNumber(1));
                msg
            },
            // Message 7: empty message (all fields missing).
            DynamicMessage::new(person_message.clone()),
        ]
    }

    /// Helper to test field combinations with or without presence tracking.
    fn test_field_combinations_helper(use_proto3_optional: bool, snapshot_name: &str) {
        let (person_name, person_proto) = create_person_descriptor(use_proto3_optional);
        let person_message = create_message_descriptor(vec![person_proto], person_name);

        let buffer = Vec::new();
        let cursor = io::Cursor::new(buffer);
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

        let channel_id = add_schema_and_channel(&mut writer, &person_message, "test_topic")
            .expect("failed to add schema and channel");

        let messages = create_test_messages(&person_message);
        for (idx, msg) in messages.iter().enumerate() {
            write_message(&mut writer, channel_id, msg, 1 + idx as u64)
                .expect("failed to write message");
        }

        let summary = writer.finish().expect("finishing writer failed");
        let buffer = writer.into_inner().into_inner();

        assert_eq!(
            summary.chunk_indexes.len(),
            1,
            "there should be only one chunk"
        );

        let chunks = run_decoder(&summary, buffer.as_slice());
        assert_eq!(chunks.len(), 1);

        insta::assert_snapshot!(snapshot_name, format_chunk(&chunks[0]));
    }

    /// Test various field combinations with proto3 optional (presence tracking).
    /// This includes messages with all fields, partial fields, and missing fields.
    #[test]
    fn test_field_combinations_with_presence_tracking() {
        test_field_combinations_helper(true, "field_combinations_with_presence_tracking");
    }

    /// Test various field combinations without proto3 optional (no presence tracking).
    /// Unset fields will show default values instead of null.
    #[test]
    fn test_field_combinations_without_presence_tracking() {
        test_field_combinations_helper(false, "field_combinations_without_presence_tracking");
    }

    /// This test verifies that we are resilient to decode failures. When messages fail to decode,
    /// they should be logged and skipped without causing length mismatches.
    #[test]
    fn test_decode_failure_resilience() {
        use prost_reflect::prost::Message as _;

        let (summary, buffer) = {
            let (person_name, person_proto) = create_person_descriptor(true);
            let person_message = create_message_descriptor(vec![person_proto], person_name);

            let buffer = Vec::new();
            let cursor = io::Cursor::new(buffer);
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let channel_id = add_schema_and_channel(&mut writer, &person_message, "test_topic")
                .expect("failed to add schema and channel");

            // Write a mix of valid messages and completely invalid protobuf data.
            for i in 0..10 {
                let bytes = if i % 2 == 0 {
                    // Write completely invalid protobuf data (random bytes that will fail to decode).
                    vec![0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xAA, 0xBB]
                } else {
                    // Write a valid message.
                    let msg = DynamicMessage::parse_text_format(
                        person_message.clone(),
                        &format!("name: \"Person{i}\" id: {i}"),
                    )
                    .expect("failed to parse text format");
                    msg.encode_to_vec()
                };

                // Write the (valid or invalid) message directly.
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

        let chunks = run_decoder(&summary, buffer.as_slice());
        assert_eq!(chunks.len(), 1);
        // We wrote 10 messages (5 valid, 5 invalid), so we should get 5 rows.
        assert_eq!(chunks[0].num_rows(), 5);

        insta::assert_snapshot!("decode_failure_resilience", format_chunk(&chunks[0]));
    }

    /// Creates a `Color` message descriptor equivalent to:
    ///
    /// ```protobuf
    /// message Color {
    ///   string object = 1;
    ///   oneof color {
    ///     string rgb = 2;
    ///     string hsv = 3;
    ///     string bgr = 4;
    ///   }
    ///   optional float gamma = 5;
    /// }
    /// ```
    ///
    /// If `set_proto3_optional_flag` is `false`, the `gamma` field will still be in a
    /// single-field oneof but without the `proto3_optional` flag. This simulates protobuf
    /// compilers/tools that don't emit the flag.
    fn create_color_descriptor(set_proto3_optional_flag: bool) -> (&'static str, DescriptorProto) {
        let color_proto = DescriptorProto {
            name: Some("Color".into()),
            field: vec![
                FieldDescriptorProto {
                    name: Some("object".into()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    ..Default::default()
                },
                // Oneof field options: rgb, hsv, or bgr.
                FieldDescriptorProto {
                    name: Some("rgb".into()),
                    number: Some(2),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    oneof_index: Some(0), // Part of real oneof "color".
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("hsv".into()),
                    number: Some(3),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    oneof_index: Some(0), // Part of real oneof "color".
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("bgr".into()),
                    number: Some(4),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::String as i32),
                    oneof_index: Some(0), // Part of real oneof "color".
                    ..Default::default()
                },
                // Proto3 optional field (synthetic oneof "_gamma").
                FieldDescriptorProto {
                    name: Some("gamma".into()),
                    number: Some(5),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Float as i32),
                    proto3_optional: set_proto3_optional_flag.then_some(true),
                    oneof_index: Some(1), // Synthetic oneof "_gamma".
                    ..Default::default()
                },
            ],
            oneof_decl: vec![
                // Real oneof (index 0).
                OneofDescriptorProto {
                    name: Some("color".into()),
                    ..Default::default()
                },
                // Synthetic oneof for proto3 optional "gamma" (index 1).
                OneofDescriptorProto {
                    name: Some("_gamma".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        ("com.example.Color", color_proto)
    }

    /// Creates a `Scene` message descriptor equivalent to:
    ///
    /// ```protobuf
    /// message Scene {
    ///   Color object = 1;
    /// }
    /// ```
    fn create_scene_descriptor() -> (&'static str, DescriptorProto) {
        let scene_proto = DescriptorProto {
            name: Some("Scene".into()),
            field: vec![
                // Regular struct field containing Color (which has oneof inside).
                FieldDescriptorProto {
                    name: Some("object".into()),
                    number: Some(1),
                    label: Some(field_descriptor_proto::Label::Optional as i32),
                    r#type: Some(field_descriptor_proto::Type::Message as i32),
                    type_name: Some("Color".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        ("com.example.Scene", scene_proto)
    }

    fn create_scene_test_messages(scene_message: &MessageDescriptor) -> Vec<DynamicMessage> {
        vec![
            // Message 1: scene with nested Color using rgb.
            DynamicMessage::parse_text_format(
                scene_message.clone(),
                "object: { object: \"cube\" rgb: \"128,64,32\" }",
            )
            .expect("failed to parse text format"),
            // Message 2: scene with nested Color using bgr.
            DynamicMessage::parse_text_format(
                scene_message.clone(),
                "object: { object: \"pyramid\" bgr: \"0,255,0\" }",
            )
            .expect("failed to parse text format"),
        ]
    }

    fn create_color_test_messages(color_message: &MessageDescriptor) -> Vec<DynamicMessage> {
        vec![
            // Message 1: object, gamma, and rgb color.
            DynamicMessage::parse_text_format(
                color_message.clone(),
                "object: \"box\" gamma: 2.2 rgb: \"255,0,0\"",
            )
            .expect("failed to parse text format"),
            // Message 2: object and hsv color (no gamma).
            DynamicMessage::parse_text_format(
                color_message.clone(),
                "object: \"sphere\" hsv: \"120,1.0,1.0\"",
            )
            .expect("failed to parse text format"),
            // Message 3: only object (no gamma, no color oneof field set).
            DynamicMessage::parse_text_format(color_message.clone(), "object: \"cone\"")
                .expect("failed to parse text format"),
        ]
    }

    /// Helper to test oneof fields with or without the `proto3_optional` flag on `gamma`.
    fn test_oneof_fields_helper(set_proto3_optional_flag: bool) {
        let (color_name, color_proto) = create_color_descriptor(set_proto3_optional_flag);
        let color_message = create_message_descriptor(vec![color_proto], color_name);

        let buffer = Vec::new();
        let cursor = io::Cursor::new(buffer);
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

        let channel_id = add_schema_and_channel(&mut writer, &color_message, "color_topic")
            .expect("failed to add schema and channel");

        let messages = create_color_test_messages(&color_message);
        for (i, msg) in messages.iter().enumerate() {
            write_message(&mut writer, channel_id, msg, (i + 1) as u64)
                .expect("failed to write message");
        }

        let summary = writer.finish().expect("finishing writer failed");
        let buffer = writer.into_inner().into_inner();

        let chunks = run_decoder(&summary, buffer.as_slice());

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].num_rows(), 3);

        insta::assert_snapshot!("oneof_fields", format_chunk(&chunks[0]));
    }

    /// This test verifies that all oneof fields (both real and synthetic) are included
    /// in the Arrow output.
    #[test]
    fn test_oneof_fields() {
        test_oneof_fields_helper(true);
    }

    /// Same as `test_oneof_fields` but without the `proto3_optional` flag on `gamma`.
    /// Verifies that the output is identical regardless of whether the flag is set.
    #[test]
    fn test_oneof_fields_without_proto3_optional_flag() {
        test_oneof_fields_helper(false);
    }

    /// This test verifies that nested oneof fields are included in the Arrow output.
    #[test]
    fn test_oneof_nested() {
        let (_, color_proto) = create_color_descriptor(true);
        let (scene_name, scene_proto) = create_scene_descriptor();
        let scene_message = create_message_descriptor(vec![color_proto, scene_proto], scene_name);

        let buffer = Vec::new();
        let cursor = io::Cursor::new(buffer);
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

        let channel_id = add_schema_and_channel(&mut writer, &scene_message, "scene_topic")
            .expect("failed to add schema and channel");

        let messages = create_scene_test_messages(&scene_message);
        for (i, msg) in messages.iter().enumerate() {
            write_message(&mut writer, channel_id, msg, (i + 1) as u64)
                .expect("failed to write message");
        }

        let summary = writer.finish().expect("finishing writer failed");
        let buffer = writer.into_inner().into_inner();

        let chunks = run_decoder(&summary, buffer.as_slice());

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].num_rows(), 2);

        insta::assert_snapshot!("oneof_nested", format_chunk(&chunks[0]));
    }
}
