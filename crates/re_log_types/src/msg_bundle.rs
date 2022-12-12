//! Structs and functions used for framing and de-framing a Rerun log message in Arrow.
//!
//! An example main message (outer) schema:
//! ```text
//! +---------------------------------------------+-----------------------------------------------------+
//! | timelines                                   | components                                          |
//! +---------------------------------------------+-----------------------------------------------------+
//! | [{timeline: log_time, type: 0, time: 1234}] | {rect: [{x: 0, y: 0, w: 0, h: 0}], color_rgba: [0]} |
//! +---------------------------------------------+-----------------------------------------------------+
//! ```
//!
//! The outer schema has precicely 2 columns: `timelines`, `components`
//! (TODO(john) do we want to add `MsgId`?)
//!
//! The `timelines` schema is *fixed* and is defined by the [`ArrowField`] implementation on
//! [`TimePoint`].
//!
//! The `components` schema is semi-flexible: it should be a [`StructArray`] with one column per
//! component. Each component schema is defined in [`crate::field_types`].

use std::collections::BTreeMap;

use anyhow::{anyhow, ensure};
use arrow2::{
    array::{Array, ListArray, StructArray},
    buffer::Buffer,
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
};
use arrow2_convert::{
    field::ArrowField,
    serialize::{ArrowSerialize, TryIntoArrow},
};

use crate::{ComponentNameRef, ObjPath, TimePoint};

//TODO(john) get rid of this eventually
const ENTITY_PATH_KEY: &str = "RERUN:entity_path";

const COL_COMPONENTS: &str = "components";
const COL_TIMELINES: &str = "timelines";

pub trait Component: ArrowField {
    /// Return the name of the component
    fn name() -> ComponentNameRef<'static>;
}

pub struct ComponentBundle<'data> {
    pub name: ComponentNameRef<'data>,
    pub field: Field,
    pub component: Box<dyn Array>,
}

pub struct MessageBundle<'data> {
    pub obj_path: ObjPath,
    pub time_point: TimePoint,
    pub components: Vec<ComponentBundle<'data>>,
}

impl<'data> MessageBundle<'data> {
    pub fn new(obj_path: ObjPath, time_point: TimePoint) -> Self {
        Self {
            obj_path,
            time_point,
            components: Vec::new(),
        }
    }

    /// Try to append a collection of `Component` onto the `MessageBundle`
    pub fn try_append_component<'a, Element, Collection>(
        &mut self,
        component: Collection,
    ) -> anyhow::Result<()>
    where
        Element: Component + ArrowSerialize + ArrowField<Type = Element> + 'static,
        Collection: IntoIterator<Item = &'a Element>,
    {
        let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(component)?;
        let wrapped = wrap_in_listarray(array).boxed();
        let field = Field::new(Element::name(), wrapped.data_type().clone(), false);

        let bundle = ComponentBundle {
            name: Element::name(),
            field,
            component: wrapped,
        };

        self.components.push(bundle);
        Ok(())
    }
}

impl<'data> TryFrom<MessageBundle<'data>> for (Schema, Chunk<Box<dyn Array>>) {
    type Error = anyhow::Error;

    /// Build a single Arrow log message from this [`MessageBundle`]
    fn try_from(bundle: MessageBundle<'data>) -> Result<Self, Self::Error> {
        let mut schema = Schema::default();
        let mut cols: Vec<Box<dyn Array>> = Vec::new();

        schema.metadata = BTreeMap::from([(ENTITY_PATH_KEY.into(), bundle.obj_path.to_string())]);

        // Build & pack timelines
        let timelines_field = Field::new(COL_TIMELINES, TimePoint::data_type(), false);
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

fn pack_components<'data>(
    components: impl Iterator<Item = ComponentBundle<'data>>,
) -> (Schema, StructArray) {
    let (component_fields, component_cols): (Vec<_>, Vec<_>) = components
        .map(
            |ComponentBundle {
                 name: _,
                 field,
                 component,
             }| (field, component.to_boxed()),
        )
        .unzip();

    let data_type = DataType::Struct(component_fields);
    let packed = StructArray::new(data_type, component_cols, None);

    let schema = Schema {
        fields: [Field::new(
            COL_COMPONENTS,
            packed.data_type().clone(),
            false,
        )]
        .to_vec(),
        ..Default::default()
    };

    (schema, packed)
}

/// Extract a [`TimePoint`] from the "timelines" column
fn extract_timelines(schema: &Schema, msg: &Chunk<Box<dyn Array>>) -> anyhow::Result<TimePoint> {
    use arrow2_convert::deserialize::arrow_array_deserialize_iterator;

    let timelines = schema
        .fields
        .iter()
        .position(|f| f.name == COL_TIMELINES) // TODO(cmc): maybe at least a constant or something
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
fn extract_components<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<ComponentBundle<'data>>> {
    let components = schema
        .fields
        .iter()
        .position(|f| f.name == COL_COMPONENTS) // TODO(cmc): maybe at least a constant or something
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

// ----------------------------------------------------------------------------

/// Wrap `field_array` in a single-element `ListArray`
pub fn wrap_in_listarray(field_array: Box<dyn Array>) -> ListArray<i32> {
    let datatype = ListArray::<i32>::default_datatype(field_array.data_type().clone());
    let offsets = Buffer::from(vec![0, field_array.len() as i32]);
    let values = field_array;
    let validity = None;
    ListArray::<i32>::from_data(datatype, offsets, values, validity)
}
