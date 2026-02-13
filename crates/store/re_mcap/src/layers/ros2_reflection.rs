use std::sync::Arc;

use anyhow::Context as _;
use arrow::array::{
    ArrayBuilder, ArrowPrimitiveType, BooleanBuilder, FixedSizeListBuilder, Float32Builder,
    Float64Builder, Int8Builder, Int16Builder, Int32Builder, Int64Builder, ListBuilder,
    PrimitiveBuilder, StringBuilder, StructBuilder, UInt8Builder, UInt16Builder, UInt32Builder,
    UInt64Builder,
};
use arrow::datatypes::{
    DataType, Field, Fields, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type,
    UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use cdr_encoding::CdrDeserializer;
use re_chunk::{Chunk, ChunkId};
use re_ros_msg::MessageSchema;
use re_ros_msg::deserialize::primitive_array::PrimitiveArray;
use re_ros_msg::deserialize::{MapResolver, MessageSeed, Value};
use re_ros_msg::message_spec::{ArraySize, BuiltInType, ComplexType, MessageSpecification, Type};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use serde::de::DeserializeSeed as _;

use crate::parsers::{MessageParser, ParserContext, dds};
use crate::{Error, LayerIdentifier, MessageLayer};

pub fn decode_bytes(top: &MessageSchema, buf: &[u8]) -> anyhow::Result<Value> {
    // 4-byte encapsulation header
    if buf.len() < 4 {
        anyhow::bail!("short encapsulation");
    }

    let representation_identifier = dds::RepresentationIdentifier::from_bytes([buf[0], buf[1]])
        .with_context(|| "failed to parse CDR representation identifier")?;

    let resolver = MapResolver::new(top.dependencies.iter().map(|dep| (dep.name.clone(), dep)));

    let seed = MessageSeed::new(&top.spec, &resolver);

    if representation_identifier.is_big_endian() {
        let mut de = CdrDeserializer::<byteorder::BigEndian>::new(&buf[4..]);
        seed.deserialize(&mut de)
            .with_context(|| "failed to deserialize CDR message")
    } else {
        let mut de = CdrDeserializer::<byteorder::LittleEndian>::new(&buf[4..]);
        seed.deserialize(&mut de)
            .with_context(|| "failed to deserialize CDR message")
    }
}

struct Ros2ReflectionMessageParser {
    message_schema: MessageSchema,
    builder: FixedSizeListBuilder<MessageStructBuilder>,
}

#[derive(Debug, thiserror::Error)]
pub enum Ros2ReflectionError {
    #[error("Invalid message on channel {channel} for schema {schema}: {source}")]
    InvalidMessage {
        schema: String,
        channel: String,
        source: anyhow::Error,
    },

    #[error("Failed to downcast builder to expected type: {0}")]
    Downcast(&'static str),
}

/// Minimal wrapper around [`StructBuilder`] that also holds the [`MessageSpecification`]
struct MessageStructBuilder {
    builder: StructBuilder,
    spec: Arc<MessageSpecification>,
}

impl ArrayBuilder for MessageStructBuilder {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_box_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn len(&self) -> usize {
        self.builder.len()
    }

    fn is_empty(&self) -> bool {
        self.builder.is_empty()
    }

    fn finish(&mut self) -> arrow::array::ArrayRef {
        Arc::new(self.builder.finish())
    }

    fn finish_cloned(&self) -> arrow::array::ArrayRef {
        Arc::new(self.builder.finish_cloned())
    }
}

impl Ros2ReflectionMessageParser {
    fn new(num_rows: usize, message_schema: MessageSchema) -> anyhow::Result<Self> {
        let struct_builder =
            struct_builder_from_message_spec(&message_schema.spec, &message_schema.dependencies)?;
        let builder = FixedSizeListBuilder::with_capacity(struct_builder, 1, num_rows);

        Ok(Self {
            message_schema,
            builder,
        })
    }
}

impl MessageParser for Ros2ReflectionMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let value = decode_bytes(&self.message_schema, msg.data.as_ref()).map_err(|err| {
            Ros2ReflectionError::InvalidMessage {
                schema: self.message_schema.spec.name.clone(),
                channel: msg.channel.topic.clone(),
                source: err,
            }
        })?;

        if let Value::Message(message_fields) = value {
            let message_struct_builder = self.builder.values();
            let spec = &message_struct_builder.spec;

            // Iterate over all struct fields based on the message spec order
            for (i, spec_field) in spec.fields.iter().enumerate() {
                if let Some(field_builder) = message_struct_builder
                    .builder
                    .field_builders_mut()
                    .get_mut(i)
                {
                    if let Some(field_value) = message_fields.get(&spec_field.name) {
                        append_value(field_builder, field_value)?;
                    } else {
                        re_log::warn_once!(
                            "Field {} is missing from message content",
                            spec_field.name
                        );
                    }
                }
            }

            message_struct_builder.builder.append(true);
            self.builder.append(true);
        } else {
            return Err(anyhow::anyhow!("Expected message value, got {value:?}"));
        }

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            message_schema,
            mut builder,
        } = *self;

        let archetype_name = message_schema.spec.name.clone().replace('/', ".");

        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            std::iter::once((
                ComponentDescriptor::partial("message").with_builtin_archetype(archetype_name),
                builder.finish().into(),
            ))
            .collect(),
        )
        .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        Ok(vec![message_chunk])
    }
}

fn downcast_builder<T: std::any::Any>(
    builder: &mut dyn ArrayBuilder,
) -> Result<&mut T, Ros2ReflectionError> {
    builder.as_any_mut().downcast_mut::<T>().ok_or_else(|| {
        let type_name = std::any::type_name::<T>();
        Ros2ReflectionError::Downcast(type_name.strip_suffix("Builder").unwrap_or(type_name))
    })
}

fn append_slice_to_list<T>(
    builder: &mut dyn ArrayBuilder,
    vec: &[T::Native],
) -> Result<(), Ros2ReflectionError>
where
    T: ArrowPrimitiveType,
    PrimitiveBuilder<T>: 'static,
{
    let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
    let values_builder = downcast_builder::<PrimitiveBuilder<T>>(list_builder.values())?;
    values_builder.append_slice(vec);
    list_builder.append(true);
    Ok(())
}

fn append_primitive_array(
    builder: &mut dyn ArrayBuilder,
    prim_array: &PrimitiveArray,
) -> Result<(), Ros2ReflectionError> {
    match prim_array {
        PrimitiveArray::Bool(vec) => {
            // `Bool` is a special case since Arrow doesn't have a primitive boolean array
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            let values_builder = downcast_builder::<BooleanBuilder>(list_builder.values())?;
            values_builder.append_slice(vec);
            list_builder.append(true);
            Ok(())
        }
        PrimitiveArray::I8(vec) => append_slice_to_list::<Int8Type>(builder, vec),
        PrimitiveArray::U8(vec) => append_slice_to_list::<UInt8Type>(builder, vec),
        PrimitiveArray::I16(vec) => append_slice_to_list::<Int16Type>(builder, vec),
        PrimitiveArray::U16(vec) => append_slice_to_list::<UInt16Type>(builder, vec),
        PrimitiveArray::I32(vec) => append_slice_to_list::<Int32Type>(builder, vec),
        PrimitiveArray::U32(vec) => append_slice_to_list::<UInt32Type>(builder, vec),
        PrimitiveArray::I64(vec) => append_slice_to_list::<Int64Type>(builder, vec),
        PrimitiveArray::U64(vec) => append_slice_to_list::<UInt64Type>(builder, vec),
        PrimitiveArray::F32(vec) => append_slice_to_list::<Float32Type>(builder, vec),
        PrimitiveArray::F64(vec) => append_slice_to_list::<Float64Type>(builder, vec),
        PrimitiveArray::String(items) => {
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            let values_builder = downcast_builder::<StringBuilder>(list_builder.values())?;
            for item in items {
                values_builder.append_value(item);
            }
            list_builder.append(true);
            Ok(())
        }
    }
}

fn append_value(builder: &mut dyn ArrayBuilder, val: &Value) -> Result<(), Ros2ReflectionError> {
    match val {
        Value::Bool(x) => downcast_builder::<BooleanBuilder>(builder)?.append_value(*x),
        Value::I8(x) => downcast_builder::<Int8Builder>(builder)?.append_value(*x),
        Value::U8(x) => downcast_builder::<UInt8Builder>(builder)?.append_value(*x),
        Value::I16(x) => downcast_builder::<Int16Builder>(builder)?.append_value(*x),
        Value::U16(x) => downcast_builder::<UInt16Builder>(builder)?.append_value(*x),
        Value::I32(x) => downcast_builder::<Int32Builder>(builder)?.append_value(*x),
        Value::U32(x) => downcast_builder::<UInt32Builder>(builder)?.append_value(*x),
        Value::I64(x) => downcast_builder::<Int64Builder>(builder)?.append_value(*x),
        Value::U64(x) => downcast_builder::<UInt64Builder>(builder)?.append_value(*x),
        Value::F32(x) => downcast_builder::<Float32Builder>(builder)?.append_value(*x),
        Value::F64(x) => downcast_builder::<Float64Builder>(builder)?.append_value(*x),
        Value::String(x) => {
            downcast_builder::<StringBuilder>(builder)?.append_value(x.clone());
        }
        Value::Message(message_fields) => {
            let message_struct_builder = downcast_builder::<MessageStructBuilder>(builder)?;
            let spec = &message_struct_builder.spec;

            // Use the specification field order to iterate through struct builder fields
            for (i, spec_field) in spec.fields.iter().enumerate() {
                if let Some(field_builder) = message_struct_builder
                    .builder
                    .field_builders_mut()
                    .get_mut(i)
                {
                    if let Some(field_value) = message_fields.get(&spec_field.name) {
                        append_value(field_builder, field_value)?;
                    } else {
                        re_log::warn_once!(
                            "Field {} is missing from message content",
                            spec_field.name
                        );
                    }
                }
            }

            message_struct_builder.builder.append(true);
        }
        Value::Array(vec) | Value::Sequence(vec) => {
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;

            for val in vec {
                append_value(list_builder.values(), val)?;
            }
            list_builder.append(true);
        }
        Value::PrimitiveArray(prim_array) | Value::PrimitiveSeq(prim_array) => {
            append_primitive_array(builder, prim_array)?;
        }
    }

    Ok(())
}

fn struct_builder_from_message_spec(
    spec: &MessageSpecification,
    dependencies: &[MessageSpecification],
) -> anyhow::Result<MessageStructBuilder> {
    let fields = spec
        .fields
        .iter()
        .map(|f| {
            Ok((
                arrow_field_from_type(&f.ty, &f.name, dependencies)?,
                arrow_builder_from_type(&f.ty, dependencies)?,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let (fields, field_builders): (Vec<Field>, Vec<Box<dyn ArrayBuilder>>) =
        fields.into_iter().unzip();

    Ok(MessageStructBuilder {
        builder: StructBuilder::new(fields, field_builders),
        spec: Arc::new(spec.clone()),
    })
}

fn arrow_builder_from_type(
    ty: &Type,
    dependencies: &[MessageSpecification],
) -> anyhow::Result<Box<dyn ArrayBuilder>> {
    Ok(match ty {
        Type::BuiltIn(p) => match p {
            BuiltInType::Bool => Box::new(BooleanBuilder::new()),
            BuiltInType::Byte | BuiltInType::UInt8 => Box::new(UInt8Builder::new()),
            BuiltInType::Char | BuiltInType::Int8 => Box::new(Int8Builder::new()),
            BuiltInType::Int16 => Box::new(Int16Builder::new()),
            BuiltInType::UInt16 => Box::new(UInt16Builder::new()),
            BuiltInType::Int32 => Box::new(Int32Builder::new()),
            BuiltInType::UInt32 => Box::new(UInt32Builder::new()),
            BuiltInType::Int64 => Box::new(Int64Builder::new()),
            BuiltInType::UInt64 => Box::new(UInt64Builder::new()),
            BuiltInType::Float32 => Box::new(Float32Builder::new()),
            BuiltInType::Float64 => Box::new(Float64Builder::new()),
            BuiltInType::String(_) | BuiltInType::WString(_) => Box::new(StringBuilder::new()),
        },
        Type::Complex(complex_type) => {
            // Look up the message spec in dependencies
            let spec = resolve_complex_type(complex_type, dependencies).ok_or_else(|| {
                anyhow::anyhow!("Could not resolve complex type: {complex_type:?}")
            })?;
            Box::new(struct_builder_from_message_spec(spec, dependencies)?)
        }
        Type::Array { ty, .. } => {
            Box::new(ListBuilder::new(arrow_builder_from_type(ty, dependencies)?))
        }
    })
}

fn arrow_field_from_type(
    ty: &Type,
    name: &str,
    dependencies: &[MessageSpecification],
) -> anyhow::Result<Field> {
    datatype_from_type(ty, dependencies).map(|data_type| Field::new(name, data_type, true))
}

fn datatype_from_type(
    ty: &Type,
    dependencies: &[MessageSpecification],
) -> anyhow::Result<DataType> {
    Ok(match ty {
        Type::BuiltIn(p) => match p {
            BuiltInType::Bool => DataType::Boolean,
            BuiltInType::Byte | BuiltInType::UInt8 => DataType::UInt8,
            BuiltInType::Char | BuiltInType::Int8 => DataType::Int8,
            BuiltInType::Int16 => DataType::Int16,
            BuiltInType::UInt16 => DataType::UInt16,
            BuiltInType::Int32 => DataType::Int32,
            BuiltInType::UInt32 => DataType::UInt32,
            BuiltInType::Int64 => DataType::Int64,
            BuiltInType::UInt64 => DataType::UInt64,
            BuiltInType::Float32 => DataType::Float32,
            BuiltInType::Float64 => DataType::Float64,
            BuiltInType::String(_) | BuiltInType::WString(_) => DataType::Utf8, // No wstring in Arrow
        },
        Type::Complex(complex_type) => {
            let spec = resolve_complex_type(complex_type, dependencies).ok_or_else(|| {
                anyhow::anyhow!("Could not resolve complex type: {complex_type:?}")
            })?;
            let fields = spec
                .fields
                .iter()
                .map(|f| arrow_field_from_type(&f.ty, &f.name, dependencies))
                .collect::<anyhow::Result<Fields>>()?;
            DataType::Struct(fields)
        }
        Type::Array { ty, size } => match size {
            ArraySize::Fixed(_) | ArraySize::Bounded(_) | ArraySize::Unbounded => {
                DataType::new_list(datatype_from_type(ty, dependencies)?, true)
            }
        },
    })
}

fn resolve_complex_type<'a>(
    complex_type: &ComplexType,
    dependencies: &'a [MessageSpecification],
) -> Option<&'a MessageSpecification> {
    dependencies.iter().find(|spec| match complex_type {
        ComplexType::Absolute { package, name } => {
            spec.name == format!("{package}/{name}") || spec.name == *name
        }
        ComplexType::Relative { name } => {
            spec.name == *name || spec.name.ends_with(&format!("/{name}"))
        }
    })
}

/// Provides reflection-based conversion of ROS2-encoded MCAP messages.
///
/// This layer dynamically parses ROS2 messages at runtime, allowing for
/// a direct arrow representation of the messages fields, similar to the protobuf layer.
#[derive(Debug, Default)]
pub struct McapRos2ReflectionLayer {
    schemas_per_topic: ahash::HashMap<String, MessageSchema>,
}

impl MessageLayer for McapRos2ReflectionLayer {
    fn identifier() -> LayerIdentifier {
        "ros2_reflection".into()
    }

    fn init(&mut self, summary: &mcap::Summary) -> Result<(), Error> {
        for channel in summary.channels.values() {
            let schema = channel
                .schema
                .as_ref()
                .ok_or(Error::NoSchema(channel.topic.clone()))?;

            if schema.encoding.as_str() != "ros2msg" {
                continue;
            }

            let schema_content = String::from_utf8_lossy(schema.data.as_ref());
            let message_schema =
                MessageSchema::parse(&schema.name, &schema_content).map_err(|err| {
                    Error::InvalidSchema {
                        schema: schema.name.clone(),
                        source: err,
                    }
                })?;

            let found = self
                .schemas_per_topic
                .insert(channel.topic.clone(), message_schema);

            re_log::debug_assert!(
                found.is_none(),
                "Duplicate schema for topic {}",
                channel.topic
            );
        }

        Ok(())
    }

    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        let Some(schema) = channel.schema.as_ref() else {
            return false;
        };

        if schema.encoding.as_str() != "ros2msg" {
            return false;
        }

        // Only support channels if the semantic layer doesn't support them
        // First check if we have parsed the schema successfully
        if !self.schemas_per_topic.contains_key(&channel.topic) {
            return false;
        }

        // Check if the semantic layer would handle this message type
        let semantic_layer = super::McapRos2Layer::new();
        !semantic_layer.supports_schema(&schema.name)
    }

    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        let message_schema = self.schemas_per_topic.get(&channel.topic)?;
        Some(Box::new(
            Ros2ReflectionMessageParser::new(num_rows, message_schema.clone()).ok()?,
        ))
    }
}
