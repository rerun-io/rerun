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
//! component. Each component schema is defined in [`crate::component_types`].

use std::collections::BTreeMap;

use arrow2::{
    array::{new_empty_array, Array, ListArray, StructArray},
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
    offset::Offsets,
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

    #[error(transparent)]
    PathParseError(#[from] PathParseError),

    #[error("Could not serialize components to Arrow")]
    ArrowSerializationError(#[from] arrow2::error::Error),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type Result<T> = std::result::Result<T, MsgBundleError>;

use crate::{
    parse_entity_path, ArrowMsg, ComponentName, EntityPath, MsgId, PathParseError, TimePoint,
};

//TODO(john) get rid of this eventually
const ENTITY_PATH_KEY: &str = "RERUN:entity_path";

const COL_COMPONENTS: &str = "components";
const COL_TIMELINES: &str = "timelines";

pub trait Component: ArrowField {
    /// The name of the component
    fn name() -> ComponentName;

    /// Create a [`Field`] for this `Component`
    fn field() -> Field {
        Field::new(Self::name().as_str(), Self::data_type(), false)
    }
}

/// A trait to identify any `Component` that is ready to be collected and subsequently serialized
/// into an Arrow payload.
pub trait SerializableComponent
where
    Self: Component + ArrowSerialize + ArrowField<Type = Self> + 'static,
{
}
impl<C> SerializableComponent for C where
    C: Component + ArrowSerialize + ArrowField<Type = C> + 'static
{
}

/// A `ComponentBundle` holds an Arrow component column, and its field name.
///
/// A `ComponentBundle` can be created from a collection of any element that implements the
/// [`Component`] and [`ArrowSerialize`] traits.
///
/// # Example
///
/// ```
/// # use re_log_types::{component_types::Point2D, msg_bundle::ComponentBundle};
/// let points = vec![Point2D { x: 0.0, y: 1.0 }];
/// let bundle = ComponentBundle::try_from(points).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ComponentBundle {
    /// The name of the Component, used as column name in the table `Field`.
    name: ComponentName,
    /// The Component payload `Array`.
    value: ListArray<i32>,
}

impl ComponentBundle {
    #[inline]
    pub fn new_empty(name: ComponentName, data_type: DataType) -> Self {
        Self {
            name,
            value: wrap_in_listarray(new_empty_array(data_type)),
        }
    }

    #[inline]
    pub fn new(name: ComponentName, value: ListArray<i32>) -> Self {
        Self { name, value }
    }

    /// Create a new `ComponentBundle` from a boxed `Array`. The `Array` must be a `ListArray<i32>`.
    #[inline]
    pub fn new_from_boxed(name: ComponentName, value: &dyn Array) -> Self {
        Self {
            name,
            value: value
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .unwrap()
                .clone(),
        }
    }

    /// Returns the datatype of the bundled component, discarding the list array that wraps it (!).
    #[inline]
    pub fn data_type(&self) -> &DataType {
        ListArray::<i32>::get_child_type(self.value.data_type())
    }

    #[inline]
    pub fn name(&self) -> ComponentName {
        self.name
    }

    /// Get the `ComponentBundle` value as a boxed `Array`.
    #[inline]
    pub fn value_boxed(&self) -> Box<dyn Array> {
        self.value.to_boxed()
    }

    /// Get the `ComponentBundle` value
    #[inline]
    pub fn value_list(&self) -> &ListArray<i32> {
        &self.value
    }

    /// Returns the number of _rows_ in this bundle, i.e. the length of the bundle.
    ///
    /// Currently always 1 as we don't yet support batch insertions.
    #[inline]
    pub fn nb_rows(&self) -> usize {
        self.value.len()
    }

    /// Returns the number of _instances_ for a given `row` in the bundle, i.e. the length of a
    /// specific row within the bundle.
    #[inline]
    pub fn nb_instances(&self, row: usize) -> Option<usize> {
        self.value.offsets().lengths().nth(row)
    }
}

impl<C: SerializableComponent> TryFrom<&[C]> for ComponentBundle {
    type Error = MsgBundleError;

    fn try_from(c: &[C]) -> Result<Self> {
        let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(c)?;
        let wrapped = wrap_in_listarray(array);
        Ok(ComponentBundle::new(C::name(), wrapped))
    }
}

impl<C: SerializableComponent> TryFrom<Vec<C>> for ComponentBundle {
    type Error = MsgBundleError;

    fn try_from(c: Vec<C>) -> Result<Self> {
        c.as_slice().try_into()
    }
}

impl<C: SerializableComponent> TryFrom<&Vec<C>> for ComponentBundle {
    type Error = MsgBundleError;

    fn try_from(c: &Vec<C>) -> Result<Self> {
        c.as_slice().try_into()
    }
}

// TODO(cmc): We'd like this, but orphan rules prevent us from having it:
//
// ```
// = note: conflicting implementation in crate `core`:
//         - impl<T, U> std::convert::TryFrom<U> for T
//           where U: std::convert::Into<T>;
// ```
//
// impl<'a, C: SerializableComponent, I: IntoIterator<Item = &'a C>> TryFrom<I> for ComponentBundle {
//     type Error = MsgBundleError;

//     fn try_from(c: I) -> Result<Self> {
//         c.as_slice().try_into()
//     }
// }

/// A `MsgBundle` holds data necessary for composing a single log message.
///
/// # Example
///
/// Create a `MsgBundle` and add a component consisting of 2 [`crate::component_types::Rect2D`] values:
/// ```
/// # use re_log_types::{component_types::Rect2D, msg_bundle::MsgBundle, MsgId, EntityPath, TimePoint};
/// let component = vec![
///     Rect2D::from_xywh(0.0, 0.0, 0.0, 0.0),
///     Rect2D::from_xywh(1.0, 1.0, 0.0, 0.0)
/// ];
/// let mut bundle = MsgBundle::new(MsgId::ZERO, EntityPath::root(), TimePoint::default(), vec![]);
/// bundle.try_append_component(&component).unwrap();
/// println!("{:?}", &bundle.components[0].value_boxed());
/// ```
///
/// The resultant Arrow [`arrow2::array::Array`] for the `Rect2D` component looks as follows:
/// ```text
/// ┌─────────────────┬──────────────────────────────┐
/// │ rerun.msg_id    ┆ rerun.rect2d                 │
/// │ ---             ┆ ---                          │
/// │ list[struct[2]] ┆ list[union[6]]               │
/// ╞═════════════════╪══════════════════════════════╡
/// │ []              ┆ [[0, 0, 0, 0], [1, 1, 0, 0]] │
/// └─────────────────┴──────────────────────────────┘
/// ```
/// The `MsgBundle` can then also be converted into an [`crate::arrow_msg::ArrowMsg`]:
///
/// ```
/// # use re_log_types::{ArrowMsg, component_types::Rect2D, msg_bundle::MsgBundle, MsgId, EntityPath, TimePoint};
/// # let mut bundle = MsgBundle::new(MsgId::ZERO, EntityPath::root(), TimePoint::default(), vec![]);
/// # bundle.try_append_component(re_log_types::datagen::build_some_rects(2).iter()).unwrap();
/// let msg: ArrowMsg = bundle.try_into().unwrap();
/// dbg!(&msg);
/// ```
///
/// And the resulting Arrow [`arrow2::array::Array`] in the [`ArrowMsg`] looks as follows:
/// ```text
/// ┌─────────────────┬────────────────────────────────────────────────────────────────┐
/// │ timelines       ┆ components                                                     │
/// │ ---             ┆ ---                                                            │
/// │ list[struct[3]] ┆ struct[2]                                                      │
/// ╞═════════════════╪════════════════════════════════════════════════════════════════╡
/// │ []              ┆ {rerun.msg_id: [], rerun.rect2d: [[0, 0, 0, 0], [1, 1, 0, 0]]} │
/// └─────────────────┴────────────────────────────────────────────────────────────────┘
/// ```
#[derive(Clone, Debug)]
pub struct MsgBundle {
    /// A unique id per [`crate::LogMsg`].
    pub msg_id: MsgId,
    pub entity_path: EntityPath,
    pub time_point: TimePoint,
    pub components: Vec<ComponentBundle>,
}

impl MsgBundle {
    /// Create a new `MsgBundle` with a pre-built Vec of [`ComponentBundle`] components.
    ///
    /// The `MsgId` will automatically be appended as a component to the given `bundles`, allowing
    /// the backend to keep track of the origin of any row of data.
    pub fn new(
        msg_id: MsgId,
        entity_path: EntityPath,
        time_point: TimePoint,
        components: Vec<ComponentBundle>,
    ) -> Self {
        let mut this = Self {
            msg_id,
            entity_path,
            time_point,
            components,
        };

        // TODO(cmc): Since we don't yet support mixing splatted data within instanced rows,
        // we need to craft an array of `MsgId`s that matches the length of the other components.
        if let Some(nb_instances) = this.nb_instances(0) {
            this.try_append_component(&vec![msg_id; nb_instances])
                .unwrap();
        }

        this
    }

    /// Try to append a collection of `Component` onto the `MessageBundle`.
    ///
    /// This first converts the component collection into an Arrow array, and then wraps it in a [`ListArray`].
    pub fn try_append_component<'a, Component, Collection>(
        &mut self,
        component: Collection,
    ) -> Result<()>
    where
        Component: SerializableComponent,
        Collection: IntoIterator<Item = &'a Component>,
    {
        let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(component)?;
        let wrapped = wrap_in_listarray(array);

        let bundle = ComponentBundle::new(Component::name(), wrapped);

        self.components.push(bundle);
        Ok(())
    }

    /// Returns the number of component collections in this bundle, i.e. the length of the bundle
    /// itself.
    #[inline]
    pub fn nb_components(&self) -> usize {
        self.components.len()
    }

    /// Returns the number of _rows_ for each component collections in this bundle, i.e. the
    /// length of each component collections.
    ///
    /// All component collections within a `MsgBundle` must share the same number of rows!
    ///
    /// Currently always 1 as we don't yet support batch insertions.
    #[inline]
    pub fn nb_rows(&self) -> usize {
        self.components.first().map_or(0, |bundle| bundle.nb_rows())
    }

    /// Returns the number of _instances_ for a given `row` in the bundle, i.e. the length of a
    /// specific row within the bundle.
    ///
    /// Since we don't yet support batch insertions and all components within a single row must
    /// have the same number of instances, we simply pick the value for the first component
    /// collection.
    #[inline]
    pub fn nb_instances(&self, row: usize) -> Option<usize> {
        self.components
            .first()
            .map_or(Some(0), |bundle| bundle.nb_instances(row))
    }

    /// Returns the index of `component` in the bundle, if it exists.
    ///
    /// This is `O(n)`.
    #[inline]
    pub fn find_component(&self, component: &ComponentName) -> Option<usize> {
        self.components
            .iter()
            .map(|bundle| bundle.name)
            .position(|name| name == *component)
    }
}

impl std::fmt::Display for MsgBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let values = self.components.iter().map(|bundle| bundle.value_boxed());
        let names = self.components.iter().map(|bundle| bundle.name.as_str());
        let table = re_format::arrow::format_table(values, names);
        f.write_fmt(format_args!(
            "MsgBundle '{}' @ {:?}:\n{table}",
            self.entity_path, self.time_point
        ))
    }
}

/// Pack the passed iterator of `ComponentBundle` into a `(Schema, StructArray)` tuple.
#[inline]
fn pack_components(components: impl Iterator<Item = ComponentBundle>) -> (Schema, StructArray) {
    let (component_fields, component_cols): (Vec<Field>, Vec<Box<dyn Array>>) = components
        .map(|bundle| {
            let ComponentBundle {
                name,
                value: component,
            } = bundle;
            (
                Field::new(name.as_str(), component.data_type().clone(), false),
                component.to_boxed(),
            )
        })
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

        let entity_path_cmp = schema
            .metadata
            .get(ENTITY_PATH_KEY)
            .ok_or(MsgBundleError::MissingEntityPath)
            .and_then(|path| {
                parse_entity_path(path.as_str()).map_err(MsgBundleError::PathParseError)
            })?;

        let time_point = extract_timelines(schema, chunk)?;
        let components = extract_components(schema, chunk)?;

        Ok(Self {
            msg_id: *msg_id,
            entity_path: entity_path_cmp.into(),
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

        schema.metadata =
            BTreeMap::from([(ENTITY_PATH_KEY.into(), bundle.entity_path.to_string())]);

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
pub fn extract_timelines(schema: &Schema, chunk: &Chunk<Box<dyn Array>>) -> Result<TimePoint> {
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
        .map(|(field, component)| {
            ComponentBundle::new_from_boxed(
                ComponentName::from(field.name.as_str()),
                component.as_ref(),
            )
        })
        .collect())
}

// ----------------------------------------------------------------------------

/// Wrap `field_array` in a single-element `ListArray`
pub fn wrap_in_listarray(field_array: Box<dyn Array>) -> ListArray<i32> {
    let datatype = ListArray::<i32>::default_datatype(field_array.data_type().clone());
    let offsets = Offsets::try_from_lengths(std::iter::once(field_array.len()))
        .unwrap()
        .into();
    let values = field_array;
    let validity = None;
    ListArray::<i32>::new(datatype, offsets, values, validity)
}

/// Helper to build a `MessageBundle` from 1 component
pub fn try_build_msg_bundle1<O, T, C0>(
    msg_id: MsgId,
    into_entity_path: O,
    into_time_point: T,
    into_bundles: C0,
) -> Result<MsgBundle>
where
    O: Into<EntityPath>,
    T: Into<TimePoint>,
    C0: TryInto<ComponentBundle>,
    MsgBundleError: From<<C0 as TryInto<ComponentBundle>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_entity_path.into(),
        into_time_point.into(),
        vec![into_bundles.try_into()?],
    ))
}

/// Helper to build a `MessageBundle` from 2 components
pub fn try_build_msg_bundle2<O, T, C0, C1>(
    msg_id: MsgId,
    into_entity_path: O,
    into_time_point: T,
    into_bundles: (C0, C1),
) -> Result<MsgBundle>
where
    O: Into<EntityPath>,
    T: Into<TimePoint>,
    C0: TryInto<ComponentBundle>,
    C1: TryInto<ComponentBundle>,
    MsgBundleError: From<<C0 as TryInto<ComponentBundle>>::Error>,
    MsgBundleError: From<<C1 as TryInto<ComponentBundle>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_entity_path.into(),
        into_time_point.into(),
        vec![into_bundles.0.try_into()?, into_bundles.1.try_into()?],
    ))
}

/// Helper to build a `MessageBundle` from 3 components
pub fn try_build_msg_bundle3<O, T, C0, C1, C2>(
    msg_id: MsgId,
    into_entity_path: O,
    into_time_point: T,
    into_bundles: (C0, C1, C2),
) -> Result<MsgBundle>
where
    O: Into<EntityPath>,
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
        into_entity_path.into(),
        into_time_point.into(),
        vec![
            into_bundles.0.try_into()?,
            into_bundles.1.try_into()?,
            into_bundles.2.try_into()?,
        ],
    ))
}
