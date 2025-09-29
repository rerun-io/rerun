use arrow::{
    array::{
        ArrayBuilder, ArrowPrimitiveType, BinaryBuilder, BooleanBuilder, FixedSizeListBuilder,
        Float32Builder, Float64Builder, Int8Builder, Int16Builder, Int32Builder, Int64Builder,
        ListBuilder, PrimitiveBuilder, StringBuilder, StructBuilder, UInt8Builder, UInt16Builder,
        UInt32Builder, UInt64Builder,
    },
    datatypes::{
        DataType, Field, Fields, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type,
        Int64Type, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
    },
};
use re_chunk::{Chunk, ChunkId};
use re_types::{ComponentDescriptor, reflection::ComponentDescriptorExt as _};

use crate::parsers::ros2msg::reflection::{
    MessageSchema,
    deserialize::primitive_array::PrimitiveArray,
    deserialize::{Value, decode_bytes},
    message_spec::{ArraySize, BuiltInType, ComplexType, MessageSpecification, Type},
};
use crate::parsers::{MessageParser, ParserContext};
use crate::{Error, LayerIdentifier, MessageLayer};

struct Ros2ReflectionMessageParser {
    message_schema: MessageSchema,
    fields: Vec<(String, FixedSizeListBuilder<Box<dyn ArrayBuilder>>)>,
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

impl Ros2ReflectionMessageParser {
    fn new(num_rows: usize, message_schema: MessageSchema) -> Self {
        let mut fields = Vec::new();

        // Build Arrow builders for each field in the message, preserving order
        for field in &message_schema.spec.fields {
            let name = field.name.clone();
            let builder = arrow_builder_from_type(&field.ty, &message_schema.dependencies);
            fields.push((
                name.clone(),
                FixedSizeListBuilder::with_capacity(builder, 1, num_rows),
            ));
        }

        Self {
            message_schema,
            fields,
        }
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
            // We always need to make sure to iterate over all our builders, adding null values whenever
            // a field is missing from the message that we received.
            for (field_name, builder) in &mut self.fields {
                if let Some(field_value) = message_fields.get(field_name) {
                    append_value(builder.values(), field_value, &self.message_schema)?;
                    builder.append(true);
                } else {
                    builder.append(false);
                }
            }
        } else {
            return Err(anyhow::anyhow!("Expected message value, got {:?}", value));
        }

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            message_schema,
            fields,
        } = *self;

        let archetype_name = message_schema.spec.name.clone().replace('/', ".");
        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            fields
                .into_iter()
                .map(|(field_name, mut builder)| {
                    (
                        ComponentDescriptor::partial(field_name)
                            .with_builtin_archetype(archetype_name.clone()),
                        builder.finish().into(),
                    )
                })
                .collect(),
        )
        .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        Ok(vec![message_chunk])
    }
}

fn downcast_err<T: std::any::Any>(
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
    let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
    let values_builder = downcast_err::<PrimitiveBuilder<T>>(list_builder.values())?;
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
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            let values_builder = downcast_err::<BooleanBuilder>(list_builder.values())?;
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
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            let values_builder = downcast_err::<StringBuilder>(list_builder.values())?;
            for item in items {
                values_builder.append_value(item);
            }
            list_builder.append(true);
            Ok(())
        }
    }
}

fn append_value(
    builder: &mut dyn ArrayBuilder,
    val: &Value,
    schema: &MessageSchema,
) -> Result<(), Ros2ReflectionError> {
    match val {
        Value::Bool(x) => downcast_err::<BooleanBuilder>(builder)?.append_value(*x),
        Value::I8(x) => downcast_err::<Int8Builder>(builder)?.append_value(*x),
        Value::U8(x) => downcast_err::<UInt8Builder>(builder)?.append_value(*x),
        Value::I16(x) => downcast_err::<Int16Builder>(builder)?.append_value(*x),
        Value::U16(x) => downcast_err::<UInt16Builder>(builder)?.append_value(*x),
        Value::I32(x) => downcast_err::<Int32Builder>(builder)?.append_value(*x),
        Value::U32(x) => downcast_err::<UInt32Builder>(builder)?.append_value(*x),
        Value::I64(x) => downcast_err::<Int64Builder>(builder)?.append_value(*x),
        Value::U64(x) => downcast_err::<UInt64Builder>(builder)?.append_value(*x),
        Value::F32(x) => downcast_err::<Float32Builder>(builder)?.append_value(*x),
        Value::F64(x) => downcast_err::<Float64Builder>(builder)?.append_value(*x),
        Value::String(x) => {
            downcast_err::<StringBuilder>(builder)?.append_value(x.clone());
        }
        Value::Message(message_fields) => {
            let struct_builder = downcast_err::<StructBuilder>(builder)?;

            // For nested messages, we need to find the matching specification from dependencies
            // Since we don't have type information here, we'll try to match by field names
            let matching_spec = find_matching_message_spec(schema, message_fields);

            if let Some(spec) = matching_spec {
                // Use the specification field order to iterate through struct builder fields
                for (i, spec_field) in spec.fields.iter().enumerate() {
                    if let Some(field_builder) = struct_builder.field_builders_mut().get_mut(i) {
                        if let Some(field_value) = message_fields.get(&spec_field.name) {
                            append_value(field_builder, field_value, schema)?;
                        } else {
                            //TODO(gijsd): Field is missing in the message, append null
                            re_log::warn_once!(
                                "Field {} is missing in the message, appending null",
                                spec_field.name
                            );
                        }
                    }
                }
            } else {
                re_log::warn_once!(
                    "Could not find matching message specification for nested message with fields: {:?}",
                    message_fields.keys().collect::<Vec<_>>()
                );
            }

            struct_builder.append(true);
        }
        Value::Array(vec) | Value::Seq(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;

            for val in vec {
                append_value(list_builder.values(), val, schema)?;
            }
            list_builder.append(true);
        }
        Value::PrimitiveArray(prim_array) | Value::PrimitiveSeq(prim_array) => {
            append_primitive_array(builder, prim_array)?;
        }
    }

    Ok(())
}

fn find_matching_message_spec<'a>(
    schema: &'a MessageSchema,
    message_fields: &'a std::collections::BTreeMap<String, Value>,
) -> Option<&'a MessageSpecification> {
    schema.dependencies.iter().find(|spec| {
        spec.fields.len() == message_fields.len()
            && spec
                .fields
                .iter()
                .all(|f| message_fields.contains_key(&f.name))
    })
}

fn struct_builder_from_message_spec(
    spec: &MessageSpecification,
    dependencies: &[MessageSpecification],
) -> StructBuilder {
    let fields = spec
        .fields
        .iter()
        .map(|f| {
            (
                arrow_field_from_type(&f.ty, &f.name, dependencies),
                arrow_builder_from_type(&f.ty, dependencies),
            )
        })
        .collect::<Vec<_>>();

    let (fields, field_builders): (Vec<Field>, Vec<Box<dyn ArrayBuilder>>) =
        fields.into_iter().unzip();

    StructBuilder::new(fields, field_builders)
}

fn arrow_builder_from_type(
    ty: &Type,
    dependencies: &[MessageSpecification],
) -> Box<dyn ArrayBuilder> {
    match ty {
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
            if let Some(spec) = resolve_complex_type(complex_type, dependencies) {
                Box::new(struct_builder_from_message_spec(spec, dependencies))
            } else {
                re_log::warn_once!("Could not resolve complex type: {:?}", complex_type);
                Box::new(BinaryBuilder::new()) // Fallback to binary
            }
        }
        Type::Array { ty, .. } => {
            Box::new(ListBuilder::new(arrow_builder_from_type(ty, dependencies)))
        }
    }
}

fn arrow_field_from_type(ty: &Type, name: &str, dependencies: &[MessageSpecification]) -> Field {
    Field::new(name, datatype_from_type(ty, dependencies), true)
}

fn datatype_from_type(ty: &Type, dependencies: &[MessageSpecification]) -> DataType {
    match ty {
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
            if let Some(spec) = resolve_complex_type(complex_type, dependencies) {
                let fields = spec
                    .fields
                    .iter()
                    .map(|f| arrow_field_from_type(&f.ty, &f.name, dependencies))
                    .collect::<Fields>();
                DataType::Struct(fields)
            } else {
                DataType::Binary // Fallback
            }
        }
        Type::Array { ty, size } => match size {
            ArraySize::Fixed(_) | ArraySize::Bounded(_) | ArraySize::Unbounded => {
                DataType::new_list(datatype_from_type(ty, dependencies), true)
            }
        },
    }
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
            debug_assert!(found.is_none());
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
        Some(Box::new(Ros2ReflectionMessageParser::new(
            num_rows,
            message_schema.clone(),
        )))
    }
}
