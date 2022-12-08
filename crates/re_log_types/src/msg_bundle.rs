use std::collections::BTreeMap;

use anyhow::{anyhow, ensure};
use arrow2::{
    array::{Array, StructArray},
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
};
use arrow2_convert::{field::ArrowField, serialize::TryIntoArrow};

use crate::{ComponentNameRef, ObjPath, TimePoint, ENTITY_PATH_KEY};

pub struct ComponentBundle<'data> {
    pub name: ComponentNameRef<'data>,
    pub field: Field,
    pub component: Box<dyn Array>,
}

//impl<'data> FromIterator< for Vec<ComponentBundle<'data>> {
//    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
//        todo!()
//    }
//}
//(ComponentNameRef<'static>, Schema, Box<dyn Array>)

pub struct MessageBundle<'data> {
    pub obj_path: ObjPath,
    pub time_point: TimePoint,
    pub components: Vec<ComponentBundle<'data>>,
}

impl<'data> TryFrom<MessageBundle<'data>> for (Schema, Chunk<Box<dyn Array>>) {
    type Error = anyhow::Error;

    /// Build a single Arrow log message from this [`MessageBundle`]
    fn try_from(bundle: MessageBundle<'data>) -> Result<Self, Self::Error> {
        let mut schema = Schema::default();
        let mut cols: Vec<Box<dyn Array>> = Vec::new();

        schema.metadata = BTreeMap::from([(ENTITY_PATH_KEY.into(), bundle.obj_path.to_string())]);

        // Build & pack timelines
        let timelines_field = Field::new("timelines", TimePoint::data_type(), false);
        let timelines_col = [bundle.time_point].try_into_arrow()?;

        schema.fields.push(timelines_field);
        cols.push(timelines_col);

        // Build & pack components
        let (components_schema, components_data) = pack_components(bundle.components.into_iter());

        schema.fields.extend(components_schema.fields);
        schema.metadata.extend(components_schema.metadata);
        cols.push(components_data.boxed());

        Ok((schema, Chunk::new(cols)))
    }
}

impl<'data> TryFrom<(Schema, &'data Chunk<Box<dyn Array>>)> for MessageBundle<'data> {
    type Error = anyhow::Error;

    fn try_from(
        (schema, chunk): (Schema, &'data Chunk<Box<dyn Array>>),
    ) -> Result<Self, Self::Error> {
        let obj_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
            .map(|path| ObjPath::from(path.as_str()))?;

        let time_point = extract_timelines(&schema, chunk)?;
        let components = extract_components(&schema, chunk)?;

        Ok(Self {
            obj_path,
            time_point,
            components,
        })
    }
}

pub fn pack_components<'data>(
    components: impl Iterator<Item = ComponentBundle<'data>>,
) -> (Schema, StructArray) {
    let (component_fields, component_cols): (Vec<_>, Vec<_>) = components
        .map(
            |ComponentBundle {
                 name: _,
                 field,
                 component,
             }| (field.clone(), component.to_boxed()),
        )
        .unzip();

    let data_type = DataType::Struct(component_fields);
    let packed = StructArray::new(data_type, component_cols, None);

    let schema = Schema {
        fields: [Field::new("components", packed.data_type().clone(), false)].to_vec(),
        ..Default::default()
    };

    (schema, packed)
}

/// Extract a [`TimePoint`] from the "timelines" column
pub fn extract_timelines(
    schema: &Schema,
    msg: &Chunk<Box<dyn Array>>,
) -> anyhow::Result<TimePoint> {
    use arrow2_convert::deserialize::arrow_array_deserialize_iterator;

    let timelines = schema
        .fields
        .iter()
        .position(|f| f.name == "timelines") // TODO(cmc): maybe at least a constant or something
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `timelines` field`"))?;

    let mut timepoints_iter = arrow_array_deserialize_iterator::<TimePoint>(timelines.as_ref())?;

    let timepoint = timepoints_iter
        .next()
        .ok_or_else(|| anyhow!("No rows in timelines."))?;

    ensure!(
        timepoints_iter.next().is_none(),
        "Expected a single TimePoint, but found more!"
    );

    Ok(timepoint)
}

/// Extract the components from the message
pub fn extract_components<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<ComponentBundle<'data>>> {
    let components = schema
        .fields
        .iter()
        .position(|f| f.name == "components") // TODO(cmc): maybe at least a constant or something
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `components` field`"))?;

    let components = components
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect component values to be `StructArray`s"))?;

    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, comp)| ComponentBundle {
            name: field.name.as_str(),
            field: field.clone(),
            component: comp.clone(),
        })
        .collect())
}
