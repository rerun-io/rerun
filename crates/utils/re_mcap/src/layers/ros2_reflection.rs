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

use crate::parsers::ros2msg::idl::{
    ArraySize, ComplexType, MessageSchema, MessageSpec, PrimitiveType, Type,
    deserializer::{Value, decode_bytes},
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

        println!("{message_schema:#?}");

        // Build Arrow builders for each field in the message, preserving order
        for field in &message_schema.spec.fields {
            let name = field.name.clone();
            let builder = arrow_builder_from_type(&field.ty, &message_schema.dependencies);
            fields.push((
                name.clone(),
                FixedSizeListBuilder::with_capacity(builder, 1, num_rows),
            ));
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

        if let Value::Message(message_fields) = value {
            // We always need to make sure to iterate over all our builders, adding null values whenever
            // a field is missing from the message that we received.
            for (field_name, builder) in &mut self.fields {
                if let Some(field_value) = message_fields.get(field_name) {
                    re_log::trace!("Field {} found in message, appending value", field_name);
                    append_value(builder.values(), field_value, &self.message_schema)?;
                    builder.append(true);
                    re_log::trace!("Field {}: Finished writing to builders", field_name);
                } else {
                    re_log::trace!("Field {} missing in message, appending null", field_name);
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
                            .with_builtin_archetype(message_schema.name.clone()),
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
            re_log::trace!("   - Append called on string with value: {:?}", x);
            downcast_err::<StringBuilder>(builder, val)?.append_value(x.clone());
        }
        Value::Message(message_fields) => {
            re_log::trace!(
                "   - Append called on message with fields: {:?}",
                message_fields.keys().collect::<Vec<_>>()
            );
            let struct_builder = downcast_err::<StructBuilder>(builder, val)?;
            re_log::trace!(
                "   - Retrieved StructBuilder with {} fields",
                struct_builder.num_fields()
            );

            // For nested messages, we need to find the matching MessageSpec from dependencies
            // Since we don't have type information here, we'll try to match by field names
            let mut matching_spec: Option<&MessageSpec> = None;

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
                        re_log::trace!(
                            "   - Processing field {} ({})",
                            ith_arrow_field,
                            field_name
                        );

                        if let Some(field_value) = message_fields.get(field_name) {
                            re_log::trace!(
                                "   - Found field ({}) with val: {:?}",
                                field_name,
                                field_value
                            );
                            append_value(field_builder, field_value, schema)?;
                            re_log::trace!(
                                "   - Written field ({}) with val: {:?}",
                                field_name,
                                field_value
                            );
                        } else {
                            re_log::trace!(
                                "   - Field {} missing in message, skipping",
                                field_name
                            );
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
            re_log::trace!("Append called on a list with {} elements", vec.len());
            let list_builder = downcast_err::<ListBuilder<Box<dyn ArrayBuilder>>>(builder, val)?;

            re_log::trace!("Retrieved ListBuilder with values type");
            for val in vec {
                append_value(list_builder.values(), val, schema)?;
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
        .map(|f| {
            (
                arrow_field_from_type(&f.ty, &f.name, dependencies),
                arrow_builder_from_type(&f.ty, dependencies),
            )
        })
        .collect::<Vec<_>>();

    let (fields, field_builders): (Vec<Field>, Vec<Box<dyn ArrayBuilder>>) =
        fields.into_iter().unzip();

    re_log::trace!(
        "Created StructBuilder for message {} with fields: {:?}",
        spec.name,
        fields.iter().map(|f| f.name()).collect::<Vec<_>>()
    );
    StructBuilder::new(fields, field_builders)
}

fn arrow_builder_from_type(ty: &Type, dependencies: &[MessageSpec]) -> Box<dyn ArrayBuilder> {
    match ty {
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
        Type::Array { ty, .. } => {
            Box::new(ListBuilder::new(arrow_builder_from_type(ty, dependencies)))
        }
    }
}

fn arrow_field_from_type(ty: &Type, name: &str, dependencies: &[MessageSpec]) -> Field {
    Field::new(name, datatype_from_type(ty, dependencies), true)
}

fn datatype_from_type(ty: &Type, dependencies: &[MessageSpec]) -> DataType {
    match ty {
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
            println!(
                "Resolving complex type: {:?}, resolved to: {:?}",
                complex_type,
                resolve_complex_type(complex_type, dependencies)
            );
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
    dependencies: &'a [MessageSpec],
) -> Option<&'a MessageSpec> {
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
            let message_schema = MessageSchema::parse(schema.name.clone(), &schema_content)
                .map_err(|err| Error::InvalidSchema {
                    schema: schema.name.clone(),
                    source: err,
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
