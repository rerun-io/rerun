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
//! The outer schema has precisely 2 columns: `timelines`, `components`
//! (TODO(john) do we want to add `MsgId`?)
//!
//! The `timelines` schema is *fixed* and is defined by the [`ArrowField`] implementation on
//! [`TimePoint`].
//!
//! The `components` schema is semi-flexible: it should be a [`StructArray`] with one column per
//! component. Each component schema is defined in [`crate::field_types`].

use std::collections::BTreeMap;

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

/// The errors that can occur when trying to convert between Arrow and `MessageBundle` types
#[derive(thiserror::Error, Debug)]
pub enum MsgBundleError {
    #[error("Could not find entity path in Arrow Schema")]
    MissingEntityPath,

    #[error("Expect top-level `timelines` field`")]
    MissingTimelinesField,

    #[error("Expect top-level `components` field`")]
    MissingComponentsField,

    #[error("No rows in timelines")]
    NoRowsInTimeline,

    #[error("Expected component values to be `StructArray`s")]
    BadComponentValues,

    #[error("Expect a single TimePoint, but found more than one")]
    MultipleTimepoints,

    #[error("Could not serialize components to Arrow")]
    ArrowSerializationError(#[from] arrow2::error::Error),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type Result<T> = std::result::Result<T, MsgBundleError>;

use crate::{ArrowMsg, ComponentName, ComponentNameRef, MsgId, ObjPath, TimePoint};

//TODO(john) get rid of this eventually
const ENTITY_PATH_KEY: &str = "RERUN:entity_path";

const COL_COMPONENTS: &str = "components";
const COL_TIMELINES: &str = "timelines";

pub trait Component: ArrowField {
    /// The name of the component
    const NAME: ComponentNameRef<'static>;
}

/// A `ComponentBundle` holds an Arrow component column, and its field name.
///
/// A `ComponentBundle` can be created from a collection of any element that implements the
/// [`Component`] and [`ArrowSerialize`] traits.
///
/// # Example
///
/// ```
/// # use re_log_types::{field_types::Point2D, msg_bundle::ComponentBundle};
/// let points = vec![Point2D { x: 0.0, y: 1.0 }];
/// let bundle = ComponentBundle::try_from(points).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ComponentBundle {
    /// The name of the Component, used as column name in the table `Field`.
    pub name: ComponentName,
    /// The Component payload `Array`.
    pub value: Box<dyn Array>,
}

impl<C> TryFrom<&[C]> for ComponentBundle
where
    C: Component + ArrowSerialize + ArrowField<Type = C> + 'static,
{
    type Error = MsgBundleError;

    fn try_from(c: &[C]) -> Result<Self> {
        let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(c)?;
        let wrapped = wrap_in_listarray(array).boxed();
        Ok(ComponentBundle {
            name: C::NAME.to_owned(),
            value: wrapped,
        })
    }
}

impl<C> TryFrom<Vec<C>> for ComponentBundle
where
    C: Component + ArrowSerialize + ArrowField<Type = C> + 'static,
{
    type Error = MsgBundleError;

    fn try_from(c: Vec<C>) -> Result<Self> {
        c.as_slice().try_into()
    }
}

impl<C> TryFrom<&Vec<C>> for ComponentBundle
where
    C: Component + ArrowSerialize + ArrowField<Type = C> + 'static,
{
    type Error = MsgBundleError;

    fn try_from(c: &Vec<C>) -> Result<Self> {
        c.as_slice().try_into()
    }
}

/// A `MsgBundle` holds data necessary for composing a single log message.
///
/// # Example
///
/// Create a `MsgBundle` and add a component consisting of 2 [`crate::field_types::Rect2D`] values:
/// ```
/// # use re_log_types::{field_types::Rect2D, msg_bundle::MsgBundle, MsgId, ObjPath, TimePoint};
/// let component = vec![
///     Rect2D { x: 0.0, y: 0.0, w: 0.0, h: 0.0, },
///     Rect2D { x: 1.0, y: 1.0, w: 0.0, h: 0.0, }
/// ];
/// let mut bundle = MsgBundle::new(MsgId::ZERO, ObjPath::root(), TimePoint::default(), vec![]);
/// bundle.try_append_component(&component).unwrap();
/// println!("{:?}", &bundle.components[0].value);
/// ```
///
/// The resultant Arrow array for the `rect2d` component looks as follows:
/// ```text
/// +------------------------------------------------------+
/// | rect2d                                               |
/// +------------------------------------------------------+
/// | [{x: 0, y: 0, w: 0, h: 0}, {x: 1, y: 1, w: 0, h: 0}] |
/// +------------------------------------------------------+
/// ```
///
/// The `MsgBundle` can then also be converted into an [`crate::arrow_msg::ArrowMsg`]:
/// ```
/// # use re_log_types::{ArrowMsg, field_types::Rect2D, msg_bundle::MsgBundle, MsgId, ObjPath, TimePoint};
/// # let mut bundle = MsgBundle::new(MsgId::ZERO, ObjPath::root(), TimePoint::default(), vec![]);
/// # bundle.try_append_component(re_log_types::datagen::build_some_rects(2).iter()).unwrap();
/// let msg: ArrowMsg = bundle.try_into().unwrap();
/// ```
///
/// And the resulting Arrow array in the `ArrowMsg` looks as follows:
/// ```text
/// +------------------------------------------+-----------------------------------------+
/// | timelines                                | components                              |
/// +------------------------------------------+-----------------------------------------+
/// | [{timeline: frame_nr, type: 1, time: 0}] | {point2d: [{x: 9.765961, y: 5.532682}]} |
/// +------------------------------------------+-----------------------------------------+
/// ```
#[derive(Debug)]
pub struct MsgBundle {
    /// A unique id per [`crate::LogMsg`].
    pub msg_id: MsgId,
    pub obj_path: ObjPath,
    pub time_point: TimePoint,
    pub components: Vec<ComponentBundle>,
}

impl MsgBundle {
    /// Create a new `MsgBundle` with a pre-build Vec of [`ComponentBundle`] components.
    pub fn new(
        msg_id: MsgId,
        obj_path: ObjPath,
        time_point: TimePoint,
        bundles: Vec<ComponentBundle>,
    ) -> Self {
        Self {
            msg_id,
            obj_path,
            time_point,
            components: bundles,
        }
    }

    /// Try to append a collection of `Component` onto the `MessageBundle`.
    ///
    /// This first converts the component collection into an Arrow array, and then wraps it in a [`ListArray`].
    pub fn try_append_component<'a, Element, Collection>(
        &mut self,
        component: Collection,
    ) -> Result<()>
    where
        Element: Component + ArrowSerialize + ArrowField<Type = Element> + 'static,
        Collection: IntoIterator<Item = &'a Element>,
    {
        let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(component)?;
        let wrapped = wrap_in_listarray(array).boxed();

        let bundle = ComponentBundle {
            name: Element::NAME.to_owned(),
            value: wrapped,
        };

        self.components.push(bundle);
        Ok(())
    }
}

/// Pack the passed iterator of `ComponentBundle` into a `(Schema, StructArray)` tuple.
#[inline]
fn pack_components(components: impl Iterator<Item = ComponentBundle>) -> (Schema, StructArray) {
    let (component_fields, component_cols): (Vec<Field>, Vec<Box<dyn Array>>) = components
        .map(
            |ComponentBundle {
                 name,
                 value: component,
             }| {
                (
                    Field::new(name, component.data_type().clone(), false),
                    component.to_boxed(),
                )
            },
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

impl TryFrom<&ArrowMsg> for MsgBundle {
    type Error = MsgBundleError;

    /// Extract a `MsgBundle` from an `ArrowMsg`.
    fn try_from(msg: &ArrowMsg) -> Result<Self> {
        let ArrowMsg {
            msg_id,
            schema,
            chunk,
        } = msg;

        let obj_path = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or(MsgBundleError::MissingEntityPath)
            .map(|path| ObjPath::from(path.as_str()))?;

        let time_point = extract_timelines(schema, chunk)?;
        let components = extract_components(schema, chunk)?;

        Ok(Self {
            msg_id: *msg_id,
            obj_path,
            time_point,
            components,
        })
    }
}

impl TryFrom<MsgBundle> for ArrowMsg {
    type Error = MsgBundleError;

    /// Build a single Arrow log message tuple from this `MsgBundle`. See the documentation on
    /// [`MsgBundle`] for details.
    fn try_from(bundle: MsgBundle) -> Result<Self> {
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

        Ok(ArrowMsg {
            msg_id: bundle.msg_id,
            schema,
            chunk: Chunk::new(cols),
        })
    }
}

/// Extract a [`TimePoint`] from the "timelines" column. This function finds the "timelines" field
/// in `chunk` and deserializes the values into a `TimePoint` using the
/// [`arrow2_convert::deserialize::ArrowDeserialize`] trait.
fn extract_timelines(schema: &Schema, chunk: &Chunk<Box<dyn Array>>) -> Result<TimePoint> {
    use arrow2_convert::deserialize::arrow_array_deserialize_iterator;

    let timelines = schema
        .fields
        .iter()
        .position(|f| f.name == COL_TIMELINES)
        .and_then(|idx| chunk.columns().get(idx))
        .ok_or(MsgBundleError::MissingTimelinesField)?;

    let mut timepoints_iter = arrow_array_deserialize_iterator::<TimePoint>(timelines.as_ref())?;

    // We take only the first result of the iterator because at this time we only support *single*
    // row messages. At some point in the future we can support batching with this.
    let timepoint = timepoints_iter
        .next()
        .ok_or(MsgBundleError::NoRowsInTimeline)?;

    if timepoints_iter.next().is_some() {
        return Err(MsgBundleError::MultipleTimepoints);
    }

    Ok(timepoint)
}

/// Extract a vector of `ComponentBundle` from the message. This is necessary since the
/// "components" schema is flexible.
fn extract_components(
    schema: &Schema,
    msg: &Chunk<Box<dyn Array>>,
) -> Result<Vec<ComponentBundle>> {
    let components = schema
        .fields
        .iter()
        .position(|f| f.name == COL_COMPONENTS)
        .and_then(|idx| msg.columns().get(idx))
        .ok_or(MsgBundleError::MissingComponentsField)?;

    let components = components
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or(MsgBundleError::BadComponentValues)?;

    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, component)| ComponentBundle {
            name: field.name.clone(),
            value: component.clone(),
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

/// Helper to build a `MessageBundle` from 1 component
pub fn try_build_msg_bundle1<O, T, C0>(
    msg_id: MsgId,
    into_obj_path: O,
    into_time_point: T,
    into_bundles: C0,
) -> Result<MsgBundle>
where
    O: Into<ObjPath>,
    T: Into<TimePoint>,
    C0: TryInto<ComponentBundle>,
    MsgBundleError: From<<C0 as TryInto<ComponentBundle>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_obj_path.into(),
        into_time_point.into(),
        vec![into_bundles.try_into()?],
    ))
}

/// Helper to build a `MessageBundle` from 2 components
pub fn try_build_msg_bundle2<O, T, C0, C1>(
    msg_id: MsgId,
    into_obj_path: O,
    into_time_point: T,
    into_bundles: (C0, C1),
) -> Result<MsgBundle>
where
    O: Into<ObjPath>,
    T: Into<TimePoint>,
    C0: TryInto<ComponentBundle>,
    C1: TryInto<ComponentBundle>,
    MsgBundleError: From<<C0 as TryInto<ComponentBundle>>::Error>,
    MsgBundleError: From<<C1 as TryInto<ComponentBundle>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_obj_path.into(),
        into_time_point.into(),
        vec![into_bundles.0.try_into()?, into_bundles.1.try_into()?],
    ))
}

/// Helper to build a `MessageBundle` from 3 components
pub fn try_build_msg_bundle3<O, T, C0, C1, C2>(
    msg_id: MsgId,
    into_obj_path: O,
    into_time_point: T,
    into_bundles: (C0, C1, C2),
) -> Result<MsgBundle>
where
    O: Into<ObjPath>,
    T: Into<TimePoint>,
    C0: TryInto<ComponentBundle>,
    C1: TryInto<ComponentBundle>,
    C2: TryInto<ComponentBundle>,
    MsgBundleError: From<<C0 as TryInto<ComponentBundle>>::Error>,
    MsgBundleError: From<<C1 as TryInto<ComponentBundle>>::Error>,
    MsgBundleError: From<<C2 as TryInto<ComponentBundle>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_obj_path.into(),
        into_time_point.into(),
        vec![
            into_bundles.0.try_into()?,
            into_bundles.1.try_into()?,
            into_bundles.2.try_into()?,
        ],
    ))
}
