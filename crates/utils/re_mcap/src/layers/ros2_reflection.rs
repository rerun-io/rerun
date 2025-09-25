use std::collections::HashSet;

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
    #[error("invalid message on channel {channel} for schema {schema}: {source}")]
    InvalidMessage {
        schema: String,
        channel: String,
        source: anyhow::Error,
    },

    #[error("expected type {expected_type}, but found value {value:?}")]
    UnexpectedValue {
        expected_type: &'static str,
        value: String,
    },

    #[error("type {0} is not supported yet")]
    UnsupportedType(&'static str),
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

        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            fields
                .into_iter()
                .map(|(field_name, mut builder)| {
                    (
                        ComponentDescriptor::partial(field_name)
                            .with_builtin_archetype(message_schema.spec.name.clone()),
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
    _val: &Value,
) -> Result<&'a mut T, Ros2ReflectionError> {
    let builder_type_name = std::any::type_name_of_val(builder).to_owned();
    builder.as_any_mut().downcast_mut::<T>().ok_or_else(|| {
        let type_name = std::any::type_name::<T>();
        Ros2ReflectionError::UnexpectedValue {
            expected_type: type_name.strip_suffix("Builder").unwrap_or(type_name),
            value: builder_type_name,
        }
    })
}

fn append_primitive_array(
    builder: &mut dyn ArrayBuilder,
    prim_array: &PrimitiveArray,
    val: &Value,
) -> Result<(), Ros2ReflectionError> {
    match prim_array {
        PrimitiveArray::Bool(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<BooleanBuilder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::I8(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<Int8Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::U8(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<UInt8Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::I16(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<Int16Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::U16(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<UInt16Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::I32(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<Int32Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::U32(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<UInt32Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::I64(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<Int64Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::U64(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<UInt64Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::F32(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<Float32Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::F64(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<Float64Builder>(list_builder.values(), val)?;
            values_builder.append_slice(vec);
            list_builder.append(true);
        }
        PrimitiveArray::String(items) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;
            let values_builder = downcast_err::<StringBuilder>(list_builder.values(), val)?;
            for item in items {
                values_builder.append_value(item);
            }
            list_builder.append(true);
        }
    }
    Ok(())
}

fn append_value(
    builder: &mut dyn ArrayBuilder,
    val: &Value,
    schema: &MessageSchema,
) -> Result<(), Ros2ReflectionError> {
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
        Value::String(x) => {
            downcast_err::<StringBuilder>(builder, val)?.append_value(x.clone());
        }
        Value::Message(message_fields) => {
            let struct_builder = downcast_err::<StructBuilder>(builder, val)?;

            // For nested messages, we need to find the matching MessageSpec from dependencies
            // Since we don't have type information here, we'll try to match by field names
            let mut matching_spec: Option<&MessageSpecification> = None;

            // Try to find a MessageSpec that has the same field names as this message
            for spec in &schema.dependencies {
                let spec_field_names: HashSet<&String> =
                    spec.fields.iter().map(|f| &f.name).collect();
                let message_field_names: HashSet<&String> = message_fields.keys().collect();

                if spec_field_names == message_field_names {
                    matching_spec = Some(spec);
                    break;
                }
            }

            if let Some(spec) = matching_spec {
                // Use the spec field order to iterate through struct builder fields
                for (ith_arrow_field, spec_field) in spec.fields.iter().enumerate() {
                    if let Some(field_builder) =
                        struct_builder.field_builders_mut().get_mut(ith_arrow_field)
                    {
                        let field_name = &spec_field.name;

                        if let Some(field_value) = message_fields.get(field_name) {
                            append_value(field_builder, field_value, schema)?;
                        } else {
                            //TODO(gijsd): Field is missing in the message, append null
                        }
                    }
                }
            } else {
                re_log::warn!(
                    "Could not find matching MessageSpec for nested message with fields: {:?}",
                    message_fields.keys().collect::<Vec<_>>()
                );
                // Fallback: use the order from message_fields.keys() - not ideal but better than crashing
                let message_field_names: Vec<&String> = message_fields.keys().collect();
                for (ith_arrow_field, field_builder) in
                    struct_builder.field_builders_mut().iter_mut().enumerate()
                {
                    if let Some(&field_name) = message_field_names.get(ith_arrow_field) {
                        if let Some(field_value) = message_fields.get(field_name) {
                            append_value(field_builder, field_value, schema)?;
                        }
                    }
                }
            }

            struct_builder.append(true);
        }
        Value::Array(vec) | Value::Seq(vec) => {
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;

            for val in vec {
                append_value(list_builder.values(), val, schema)?;
            }
            list_builder.append(true);
        }
        Value::PrimitiveArray(prim_array) | Value::PrimitiveSeq(prim_array) => {
            append_primitive_array(builder, prim_array, val)?;
        }
    }

    Ok(())
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
