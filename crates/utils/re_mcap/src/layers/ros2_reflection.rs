use std::collections::BTreeMap;

use arrow::{
    array::{
        ArrayBuilder, BinaryBuilder, BooleanBuilder, FixedSizeListBuilder, Float32Builder,
        Float64Builder, Int8Builder, Int16Builder, Int32Builder, Int64Builder, ListBuilder,
        StringBuilder, StructBuilder, UInt8Builder, UInt16Builder, UInt32Builder, UInt64Builder,
    },
    datatypes::{DataType, Field, Fields},
};
use re_chunk::{Chunk, ChunkId};
use re_types::{ComponentDescriptor, reflection::ComponentDescriptorExt as _};

use crate::parsers::ros2msg::idl::{
    ArraySize, ComplexType, MessageSchema, MessageSpec, PrimitiveType, Type,
    deserializer::{Value, decode_bytes},
};
use crate::parsers::{MessageParser, ParserContext};
use crate::{Error, LayerIdentifier, MessageLayer};

struct Ros2ReflectionMessageParser {
    message_schema: MessageSchema,
    fields: BTreeMap<String, FixedSizeListBuilder<Box<dyn ArrayBuilder>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Ros2ReflectionError {
    #[error("invalid message on channel {channel} for schema {schema}: {source}")]
    InvalidMessage {
        schema: String,
        channel: String,
        source: anyhow::Error,
    },

    #[error("expected type {expected_type}, but found value {value:?}")]
    UnexpectedValue {
        expected_type: &'static str,
        value: Value,
    },

    #[error("type {0} is not supported yet")]
    UnsupportedType(&'static str),
}

impl Ros2ReflectionMessageParser {
    fn new(num_rows: usize, message_schema: MessageSchema) -> Self {
        let mut fields = BTreeMap::new();

        // Build Arrow builders for each field in the message
        for field in &message_schema.spec.fields {
            let name = field.name.clone();
            let builder = arrow_builder_from_type(&field.ty, &message_schema.dependencies);
            fields.insert(
                name,
                FixedSizeListBuilder::with_capacity(builder, 1, num_rows),
            );
            re_log::trace!("Added Arrow builder for field: {}", field.name);
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
                schema: self.message_schema.name.clone(),
                channel: msg.channel.topic.clone(),
                source: err,
            }
        })?;

        println!("{}: {:#?}", msg.channel.topic, value);

        // if let Value::Message(message_fields) = value {
        //     // Iterate over all our builders, adding null values for missing fields
        //     for (field_name, builder) in &mut self.fields {
        //         if let Some(field_value) = message_fields.get(field_name) {
        //             append_value(builder.values(), field_value)?;
        //             builder.append(true);
        //         } else {
        //             append_null_value(builder.values(), field_name, &self.message_schema.spec)?;
        //             builder.append(false);
        //         }
        //     }
        // } else {
        //     return Err(anyhow::anyhow!("Expected message value, got {:?}", value));
        // }

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

        return Ok(vec![]);
    }
}

fn downcast_err<'a, T: std::any::Any>(
    builder: &'a mut dyn ArrayBuilder,
    val: &Value,
) -> Result<&'a mut T, Ros2ReflectionError> {
    builder.as_any_mut().downcast_mut::<T>().ok_or_else(|| {
        let type_name = std::any::type_name::<T>();
        Ros2ReflectionError::UnexpectedValue {
            expected_type: type_name.strip_suffix("Builder").unwrap_or(type_name),
            value: val.clone(),
        }
    })
}

fn append_null_value(
    builder: &mut dyn ArrayBuilder,
    field_name: &str,
    message_spec: &MessageSpec,
) -> Result<(), Ros2ReflectionError> {
    // Find the field type in the message spec to determine what kind of null value to append
    if let Some(field) = message_spec.fields.iter().find(|f| f.name == field_name) {
        append_null_value_for_type(builder, &field.ty)?;
    } else {
        re_log::warn!("Field {} not found in message spec", field_name);
    }
    Ok(())
}

fn append_null_value_for_type(
    builder: &mut dyn ArrayBuilder,
    field_type: &Type,
) -> Result<(), Ros2ReflectionError> {
    match field_type {
        Type::Primitive(p) => match p {
            PrimitiveType::Bool => {
                downcast_err::<BooleanBuilder>(builder, &Value::Bool(false))?.append_null()
            }
            PrimitiveType::Byte | PrimitiveType::UInt8 => {
                downcast_err::<UInt8Builder>(builder, &Value::U8(0))?.append_null()
            }
            PrimitiveType::Char | PrimitiveType::Int8 => {
                downcast_err::<Int8Builder>(builder, &Value::I8(0))?.append_null()
            }
            PrimitiveType::Int16 => {
                downcast_err::<Int16Builder>(builder, &Value::I16(0))?.append_null()
            }
            PrimitiveType::UInt16 => {
                downcast_err::<UInt16Builder>(builder, &Value::U16(0))?.append_null()
            }
            PrimitiveType::Int32 => {
                downcast_err::<Int32Builder>(builder, &Value::I32(0))?.append_null()
            }
            PrimitiveType::UInt32 => {
                downcast_err::<UInt32Builder>(builder, &Value::U32(0))?.append_null()
            }
            PrimitiveType::Int64 => {
                downcast_err::<Int64Builder>(builder, &Value::I64(0))?.append_null()
            }
            PrimitiveType::UInt64 => {
                downcast_err::<UInt64Builder>(builder, &Value::U64(0))?.append_null()
            }
            PrimitiveType::Float32 => {
                downcast_err::<Float32Builder>(builder, &Value::F32(0.0))?.append_null()
            }
            PrimitiveType::Float64 => {
                downcast_err::<Float64Builder>(builder, &Value::F64(0.0))?.append_null()
            }
        },
        Type::String(_) => {
            downcast_err::<StringBuilder>(builder, &Value::String("".to_string()))?.append_null()
        }
        Type::Array { .. } => {
            // For arrays, append an empty list
            let list_builder =
                downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, &Value::Array(vec![]))?;
            list_builder.append(false); // Empty list
        }
        Type::Complex(_) => {
            // For complex types, just append null - we don't support nested messages anyway
            re_log::trace!("Appending null for unsupported complex type");
        }
    }
    Ok(())
}

fn append_value(builder: &mut dyn ArrayBuilder, val: &Value) -> Result<(), Ros2ReflectionError> {
    match val {
        Value::Bool(x) => downcast_err::<BooleanBuilder>(builder, val)?.append_value(*x),
        Value::I8(x) => downcast_err::<Int8Builder>(builder, val)?.append_value(*x),
        Value::U8(x) => downcast_err::<UInt8Builder>(builder, val)?.append_value(*x),
        Value::I16(x) => downcast_err::<Int16Builder>(builder, val)?.append_value(*x),
        Value::U16(x) => downcast_err::<UInt16Builder>(builder, val)?.append_value(*x),
        Value::I32(x) => downcast_err::<Int32Builder>(builder, val)?.append_value(*x),
        Value::U32(x) => downcast_err::<UInt32Builder>(builder, val)?.append_value(*x),
        Value::I64(x) => downcast_err::<Int64Builder>(builder, val)?.append_value(*x),
        Value::U64(x) => downcast_err::<UInt64Builder>(builder, val)?.append_value(*x),
        Value::F32(x) => downcast_err::<Float32Builder>(builder, val)?.append_value(*x),
        Value::F64(x) => downcast_err::<Float64Builder>(builder, val)?.append_value(*x),
        Value::String(x) => downcast_err::<StringBuilder>(builder, val)?.append_value(x.clone()),
        Value::Message(_message_fields) => {
            re_log::error_once!("Nested messages are not supported yet");
            return Ok(());
        }
        Value::Array(vec) | Value::Seq(vec) => {
            re_log::trace!("Append called on a list with {} elements", vec.len());
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;

            for val in vec {
                append_value(list_builder.values(), val)?;
            }
            list_builder.append(true);
            re_log::trace!("Finished append on list with {} elements", vec.len());
        }
    }

    Ok(())
}

fn struct_builder_from_message_spec(
    spec: &MessageSpec,
    dependencies: &[MessageSpec],
) -> StructBuilder {
    let fields = spec
        .fields
        .iter()
        .map(|f| arrow_field_from_type(&f.ty, &f.name, dependencies))
        .collect::<Fields>();

    let field_builders = spec
        .fields
        .iter()
        .map(|f| arrow_builder_from_type(&f.ty, dependencies))
        .collect::<Vec<_>>();

    debug_assert_eq!(fields.len(), field_builders.len());

    re_log::trace!(
        "Created StructBuilder for message {} with fields: {:?}",
        spec.name,
        fields.iter().map(|f| f.name()).collect::<Vec<_>>()
    );
    StructBuilder::new(fields, field_builders)
}

fn arrow_builder_from_type(ty: &Type, dependencies: &[MessageSpec]) -> Box<dyn ArrayBuilder> {
    let inner: Box<dyn ArrayBuilder> = match ty {
        Type::Primitive(p) => match p {
            PrimitiveType::Bool => Box::new(BooleanBuilder::new()),
            PrimitiveType::Byte | PrimitiveType::UInt8 => Box::new(UInt8Builder::new()),
            PrimitiveType::Char | PrimitiveType::Int8 => Box::new(Int8Builder::new()),
            PrimitiveType::Int16 => Box::new(Int16Builder::new()),
            PrimitiveType::UInt16 => Box::new(UInt16Builder::new()),
            PrimitiveType::Int32 => Box::new(Int32Builder::new()),
            PrimitiveType::UInt32 => Box::new(UInt32Builder::new()),
            PrimitiveType::Int64 => Box::new(Int64Builder::new()),
            PrimitiveType::UInt64 => Box::new(UInt64Builder::new()),
            PrimitiveType::Float32 => Box::new(Float32Builder::new()),
            PrimitiveType::Float64 => Box::new(Float64Builder::new()),
        },
        Type::String(_) => Box::new(StringBuilder::new()),
        Type::Complex(complex_type) => {
            // Look up the message spec in dependencies
            if let Some(spec) = resolve_complex_type(complex_type, dependencies) {
                Box::new(struct_builder_from_message_spec(spec, dependencies))
            } else {
                re_log::warn_once!("Could not resolve complex type: {:?}", complex_type);
                Box::new(BinaryBuilder::new()) // Fallback to binary
            }
        }
        Type::Array { ty, .. } => match ty.as_ref() {
            Type::String(_) => Box::new(ListBuilder::new(StringBuilder::new())),
            _ => Box::new(ListBuilder::new(arrow_builder_from_type(ty, dependencies))),
        },
    };

    inner
}

fn arrow_field_from_type(ty: &Type, name: &str, dependencies: &[MessageSpec]) -> Field {
    Field::new(name, datatype_from_type(ty, dependencies), true)
}

fn datatype_from_type(ty: &Type, dependencies: &[MessageSpec]) -> DataType {
    let inner = match ty {
        Type::Primitive(p) => match p {
            PrimitiveType::Bool => DataType::Boolean,
            PrimitiveType::Byte | PrimitiveType::UInt8 => DataType::UInt8,
            PrimitiveType::Char | PrimitiveType::Int8 => DataType::Int8,
            PrimitiveType::Int16 => DataType::Int16,
            PrimitiveType::UInt16 => DataType::UInt16,
            PrimitiveType::Int32 => DataType::Int32,
            PrimitiveType::UInt32 => DataType::UInt32,
            PrimitiveType::Int64 => DataType::Int64,
            PrimitiveType::UInt64 => DataType::UInt64,
            PrimitiveType::Float32 => DataType::Float32,
            PrimitiveType::Float64 => DataType::Float64,
        },
        Type::String(_) => DataType::Utf8,
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
    };

    inner
}

fn resolve_complex_type<'a>(
    complex_type: &ComplexType,
    dependencies: &'a [MessageSpec],
) -> Option<&'a MessageSpec> {
    dependencies.iter().find(|spec| match complex_type {
        ComplexType::Absolute { package, name } => {
            spec.name == format!("{}/{}", package, name) || spec.name == *name
        }
        ComplexType::Relative { name } => {
            spec.name == *name || spec.name.ends_with(&format!("/{}", name))
        }
    })
}

/// Provides reflection-based conversion of ROS2-encoded MCAP messages.
///
/// This layer uses the IDL deserializer to provide dynamic parsing of ROS2 messages
/// without requiring pre-compiled message definitions. It results in a direct Arrow
/// representation of the message fields, similar to the protobuf layer.
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
            let message_schema = MessageSchema::parse(schema.name.clone(), &schema_content)
                .map_err(|err| Error::InvalidSchema {
                    schema: schema.name.clone(),
                    source: err.into(),
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

        self.schemas_per_topic.contains_key(&channel.topic)
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
