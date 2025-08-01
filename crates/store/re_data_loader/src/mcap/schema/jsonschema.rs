use std::{collections::BTreeMap, sync::Arc};

use arrow::{
    array::{
        ArrayBuilder, BooleanBuilder, FixedSizeListBuilder, Float64Builder, Int64Builder,
        ListBuilder, MapBuilder, NullArray, NullBuilder, StringBuilder, StructBuilder,
    },
    datatypes::{DataType, Field},
};
use mcap::Schema;
use re_chunk::{Chunk, ChunkId};
use re_types::ComponentDescriptor;
use serde_json::Value;

use crate::mcap::decode::{McapMessageParser, PluginError};

pub struct JsonMessageParser {
    object: FixedSizeListBuilder<Box<dyn ArrayBuilder>>,
    archetype: String,
}

impl JsonMessageParser {
    pub fn new(num_rows: usize, schema: &Arc<Schema<'_>>) -> Self {
        let jsonschema: Value = serde_json::from_slice(schema.data.as_ref()).unwrap();

        dbg!(&jsonschema);

        // TODO: differentiate between arrays, objects, ...
        let object = FixedSizeListBuilder::with_capacity(
            arrow_builder_from_schema(&jsonschema),
            1,
            num_rows,
        );

        Self {
            archetype: schema.name.clone(),
            object,
        }
    }
}

impl McapMessageParser for JsonMessageParser {
    fn append(
        &mut self,
        _ctx: &mut crate::mcap::decode::ParserContext,
        msg: &mcap::Message<'_>,
    ) -> anyhow::Result<()> {
        let msg: Value = serde_json::from_slice(msg.data.as_ref()).unwrap();

        dbg!(&msg);

        let struct_builder = (&mut self.object)
            .values()
            .as_any_mut()
            .downcast_mut::<StructBuilder>()
            .unwrap();
        let clientX = struct_builder
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(
                msg.get("clientX")
                    .and_then(|x| x.as_number())
                    .and_then(|n| n.as_f64())
                    .unwrap_or_default(),
            );
        let clientY = struct_builder
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(
                msg.get("clientY")
                    .and_then(|x| x.as_number())
                    .and_then(|n| n.as_f64())
                    .unwrap_or_default(),
            );
        struct_builder.append(true);
        self.object.append(true);

        Ok(())
    }

    fn finalize(
        self: Box<Self>,
        ctx: crate::mcap::decode::ParserContext,
    ) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            mut object,
            archetype,
        } = *self;

        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            std::iter::once((
                ComponentDescriptor::partial("root").with_archetype(archetype.as_str().into()),
                object.finish().into(),
            ))
            .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        Ok(vec![message_chunk])
    }
}

fn arrow_builder_from_schema(schema_val: &Value) -> Box<dyn ArrayBuilder> {
    match schema_val {
        Value::Null => todo!(),
        Value::Bool(_) => todo!(),
        Value::Number(_) => todo!(),
        Value::String(_) => todo!(),
        Value::Array(_) => {
            todo!()
        }
        Value::Object(map) => {
            let typ = map.get("type").and_then(|v| v.as_str()).unwrap();
            match typ {
                "object" => {
                    let Some(Value::Object(properties)) = map.get("properties") else {
                        panic!("bad schema");
                    };

                    let (fields, field_builders) = (properties.iter().map(|(field_name, val)| {
                        (
                            Field::new(field_name, datatype_from(val), true),
                            arrow_builder_from_schema(val),
                        )
                    }))
                    .unzip::<_, _, Vec<_>, Vec<_>>();

                    Box::new(StructBuilder::new(fields, field_builders))
                }
                "number" => Box::new(Float64Builder::new()),
                _ => Box::new(NullBuilder::new()), // proper error handling
            }
        }
    }
}

fn datatype_from(val: &Value) -> DataType {
    match val.get("type").and_then(|t| t.as_str()) {
        Some("number") => DataType::Float64,
        _ => todo!("{} is unsupported", val),
    }
}
