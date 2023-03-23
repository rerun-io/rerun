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
    array::{Array, ListArray, StructArray},
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
    offset::Offsets,
};
use arrow2_convert::{
    deserialize::{ArrowArray, ArrowDeserialize},
    field::ArrowField,
    serialize::{ArrowSerialize, TryIntoArrow},
};

use crate::{
    parse_entity_path, ArrowMsg, ComponentName, DataCell, DataCellError, EntityPath, MsgId,
    PathParseError, TimePoint,
};

// ---

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

    #[error(transparent)]
    DataCell(#[from] DataCellError),

    // Needed to handle TryFrom<T> -> T
    #[error("Infallible")]
    Unreachable(#[from] std::convert::Infallible),
}

pub type Result<T> = std::result::Result<T, MsgBundleError>;

// ---

//TODO(john) get rid of this eventually
const ENTITY_PATH_KEY: &str = "RERUN:entity_path";

const COL_COMPONENTS: &str = "components";
const COL_TIMELINES: &str = "timelines";

// TODO(#1619): why is Component declared here?

/// A type that can used as a Component of an Entity.
///
/// Examples of components include positions and colors.
pub trait Component: ArrowField {
    /// The name of the component.
    fn name() -> ComponentName;

    /// Create a [`Field`] for this [`Component`].
    fn field() -> Field {
        Field::new(Self::name().as_str(), Self::data_type(), false)
    }
}

// TODO(#1694): do a pass over these traits, this is incomprehensible... also why would a component
// ever not be (de)serializable? Do keep in mind the whole (component, datatype) story though.

/// A [`Component`] that fulfils all the conditions required to be serialized as an Arrow payload.
pub trait SerializableComponent<ArrowFieldType = Self>
where
    Self: Component + ArrowSerialize + ArrowField<Type = Self> + 'static,
{
}

impl<C> SerializableComponent for C where
    C: Component + ArrowSerialize + ArrowField<Type = C> + 'static
{
}

/// A [`Component`] that fulfils all the conditions required to be deserialized from an Arrow
/// payload.
///
/// Note that due to the use of HRTBs in `arrow2_convert` traits, you will still need an extra HRTB
/// clause when marking a type as `DeserializableComponent`:
/// ```ignore
/// where
///     T: SerializableComponent,
///     for<'a> &'a T::ArrayType: IntoIterator,
/// ```
pub trait DeserializableComponent<ArrowFieldType = Self>
where
    Self: Component,
    Self: ArrowDeserialize + ArrowField<Type = ArrowFieldType> + 'static,
    Self::ArrayType: ArrowArray,
    for<'b> &'b Self::ArrayType: IntoIterator,
{
}

impl<C> DeserializableComponent for C
where
    C: Component,
    C: ArrowDeserialize + ArrowField<Type = C> + 'static,
    C::ArrayType: ArrowArray,
    for<'b> &'b C::ArrayType: IntoIterator,
{
}

// ---

/// A `MsgBundle` holds data necessary for composing a single log message.
#[derive(Clone, Debug)]
pub struct MsgBundle {
    /// A unique id per [`crate::LogMsg`].
    pub msg_id: MsgId,
    pub entity_path: EntityPath,
    pub time_point: TimePoint,
    pub cells: Vec<DataCell>,
}

impl MsgBundle {
    /// Create a new `MsgBundle` with a pre-built Vec of [`DataCell`] components.
    ///
    /// The `MsgId` will automatically be appended as a component to the given `bundles`, allowing
    /// the backend to keep track of the origin of any row of data.
    pub fn new(
        msg_id: MsgId,
        entity_path: EntityPath,
        time_point: TimePoint,
        components: Vec<DataCell>,
    ) -> Self {
        let mut this = Self {
            msg_id,
            entity_path,
            time_point,
            cells: components,
        };

        // TODO(cmc): Since we don't yet support mixing splatted data within instanced rows,
        // we need to craft an array of `MsgId`s that matches the length of the other components.
        this.cells.push(DataCell::from_native(
            vec![msg_id; this.num_instances()].iter(),
        ));

        this
    }

    /// Returns the number of component collections in this bundle, i.e. the length of the bundle
    /// itself.
    #[inline]
    pub fn num_components(&self) -> usize {
        self.cells.len()
    }

    /// Returns the number of _instances_ for a given `row` in the bundle, i.e. the length of a
    /// specific row within the bundle.
    ///
    /// Since we don't yet support batch insertions and all components within a single row must
    /// have the same number of instances, we simply pick the value for the first component
    /// collection.
    #[inline]
    pub fn num_instances(&self) -> usize {
        self.cells
            .first()
            .map_or(0, |cell| cell.num_instances() as _)
    }

    /// Returns the index of `component` in the bundle, if it exists.
    ///
    /// This is `O(n)`.
    #[inline]
    pub fn find_component(&self, component: &ComponentName) -> Option<usize> {
        self.cells
            .iter()
            .map(|cell| cell.component())
            .position(|name| name == *component)
    }
}

impl std::fmt::Display for MsgBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let values = self.cells.iter().map(|cell| cell.as_arrow_ref());
        let names = self.cells.iter().map(|cell| cell.component().as_str());
        let table = re_format::arrow::format_table(values, names);
        f.write_fmt(format_args!(
            "MsgBundle '{}' @ {:?}:\n{table}",
            self.entity_path, self.time_point
        ))
    }
}

/// Pack the passed iterator of `DataCell` into a `(Schema, StructArray)` tuple.
#[inline]
fn pack_components(cells: impl Iterator<Item = DataCell>) -> (Schema, StructArray) {
    let (component_fields, component_cols): (Vec<Field>, Vec<Box<dyn Array>>) = cells
        .map(|cell| {
            // NOTE: wrap in a ListArray to emulate the presence of rows, this'll go away with
            // batching.
            let data = cell.as_arrow_monolist();
            (
                Field::new(cell.component().as_str(), data.data_type().clone(), false),
                data,
            )
        })
        .unzip();

    let data_type = DataType::Struct(component_fields);
    let packed = StructArray::new(data_type, component_cols, None);

    let schema = Schema {
        fields: [
            Field::new(COL_COMPONENTS, packed.data_type().clone(), false), //
        ]
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
            cells: components,
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
        let (components_schema, components_data) = pack_components(bundle.cells.into_iter());

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

/// Extract a vector of `DataCell` from the message. This is necessary since the
/// "components" schema is flexible.
fn extract_components(schema: &Schema, msg: &Chunk<Box<dyn Array>>) -> Result<Vec<DataCell>> {
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
            // NOTE: unwrap the ListArray layer that we added during packing in order to emulate
            // the presence of rows, this'll go away with batching.
            let component = component
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .unwrap()
                .values();
            DataCell::from_arrow(ComponentName::from(field.name.as_str()), component.clone())
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
    into_cells: C0,
) -> Result<MsgBundle>
where
    O: Into<EntityPath>,
    T: Into<TimePoint>,
    C0: TryInto<DataCell>,
    MsgBundleError: From<<C0 as TryInto<DataCell>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_entity_path.into(),
        into_time_point.into(),
        vec![into_cells.try_into()?],
    ))
}

/// Helper to build a `MessageBundle` from 2 components
pub fn try_build_msg_bundle2<O, T, C0, C1>(
    msg_id: MsgId,
    into_entity_path: O,
    into_time_point: T,
    into_cells: (C0, C1),
) -> Result<MsgBundle>
where
    O: Into<EntityPath>,
    T: Into<TimePoint>,
    C0: TryInto<DataCell>,
    C1: TryInto<DataCell>,
    MsgBundleError: From<<C0 as TryInto<DataCell>>::Error>,
    MsgBundleError: From<<C1 as TryInto<DataCell>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_entity_path.into(),
        into_time_point.into(),
        vec![into_cells.0.try_into()?, into_cells.1.try_into()?],
    ))
}

/// Helper to build a `MessageBundle` from 3 components
pub fn try_build_msg_bundle3<O, T, C0, C1, C2>(
    msg_id: MsgId,
    into_entity_path: O,
    into_time_point: T,
    into_cells: (C0, C1, C2),
) -> Result<MsgBundle>
where
    O: Into<EntityPath>,
    T: Into<TimePoint>,
    C0: TryInto<DataCell>,
    C1: TryInto<DataCell>,
    C2: TryInto<DataCell>,
    MsgBundleError: From<<C0 as TryInto<DataCell>>::Error>,
    MsgBundleError: From<<C1 as TryInto<DataCell>>::Error>,
    MsgBundleError: From<<C2 as TryInto<DataCell>>::Error>,
{
    Ok(MsgBundle::new(
        msg_id,
        into_entity_path.into(),
        into_time_point.into(),
        vec![
            into_cells.0.try_into()?,
            into_cells.1.try_into()?,
            into_cells.2.try_into()?,
        ],
    ))
}
