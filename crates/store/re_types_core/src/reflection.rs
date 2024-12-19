//! Run-time reflection for reading meta-data about components and archetypes.

use arrow::array::ArrayRef;

use crate::{ArchetypeName, ComponentName};

/// A trait for code-generated enums.
pub trait Enum:
    Sized + Copy + Clone + std::hash::Hash + PartialEq + Eq + std::fmt::Display + 'static
{
    /// All variants, in the order they appear in the enum.
    fn variants() -> &'static [Self];

    /// Markdown docstring for the given enum variant.
    fn docstring_md(self) -> &'static str;
}

/// Runtime reflection about components and archetypes.
#[derive(Clone, Debug, Default)]
pub struct Reflection {
    pub components: ComponentReflectionMap,
    pub archetypes: ArchetypeReflectionMap,
}

impl Reflection {
    /// Find an [`ArchetypeReflection`] based on its short name.
    ///
    /// Useful when the only information available is the short name, e.g. when inferring archetype
    /// names from an indicator component.
    //TODO(#6889): tagged component will contain a fully qualified archetype name, so this function
    // will be unnecessary.
    pub fn archetype_reflection_from_short_name(
        &self,
        short_name: &str,
    ) -> Option<&ArchetypeReflection> {
        // note: this mirrors `ArchetypeName::short_name`'s implementation
        self.archetypes
            .get(&ArchetypeName::from(short_name))
            .or_else(|| {
                self.archetypes.get(&ArchetypeName::from(format!(
                    "rerun.archetypes.{short_name}"
                )))
            })
            .or_else(|| {
                self.archetypes.get(&ArchetypeName::from(format!(
                    "rerun.blueprint.archetypes.{short_name}"
                )))
            })
            .or_else(|| {
                self.archetypes
                    .get(&ArchetypeName::from(format!("rerun.{short_name}")))
            })
    }
}

/// Computes a placeholder for a given arrow datatype.
///
/// With the exception of a few unsupported types,
/// a placeholder is an array of the given datatype with a single element.
/// This single element is (recursively if necessary) a sort of arbitrary zero value
/// which can be used as a starting point.
/// E.g. the default for a an integer array is an array containing a single zero.
///
/// For unsupported types this yields an empty array instead.
///
/// See also [`ComponentReflection::custom_placeholder`].
pub fn generic_placeholder_for_datatype(
    datatype: &arrow2::datatypes::DataType,
) -> Box<dyn arrow2::array::Array> {
    use arrow2::{
        array,
        datatypes::{DataType, IntervalUnit},
        types,
    };

    match datatype {
        DataType::Null => Box::new(array::NullArray::new(datatype.clone(), 1)),
        DataType::Boolean => Box::new(array::BooleanArray::from_slice([false])),
        DataType::Int8 => Box::new(array::Int8Array::from_slice([0])),
        DataType::Int16 => Box::new(array::Int16Array::from_slice([0])),

        DataType::Int32
        | DataType::Date32
        | DataType::Time32(_)
        | DataType::Interval(IntervalUnit::YearMonth) => {
            // TODO(andreas): Do we have to further distinguish these types? They do share the physical type.
            Box::new(array::Int32Array::from_slice([0]))
        }
        DataType::Int64
        | DataType::Date64
        | DataType::Timestamp(_, _)
        | DataType::Time64(_)
        | DataType::Duration(_) => {
            // TODO(andreas): Do we have to further distinguish these types? They do share the physical type.
            Box::new(array::Int64Array::from_slice([0]))
        }

        DataType::UInt8 => Box::new(array::UInt8Array::from_slice([0])),
        DataType::UInt16 => Box::new(array::UInt16Array::from_slice([0])),
        DataType::UInt32 => Box::new(array::UInt32Array::from_slice([0])),
        DataType::UInt64 => Box::new(array::UInt64Array::from_slice([0])),
        DataType::Float16 => Box::new(array::Float16Array::from_slice([types::f16::from_f32(0.0)])),
        DataType::Float32 => Box::new(array::Float32Array::from_slice([0.0])),
        DataType::Float64 => Box::new(array::Float64Array::from_slice([0.0])),

        DataType::Interval(IntervalUnit::DayTime) => {
            Box::new(array::DaysMsArray::from_slice([types::days_ms::new(0, 0)]))
        }
        DataType::Interval(IntervalUnit::MonthDayNano) => {
            Box::new(array::MonthsDaysNsArray::from_slice([
                types::months_days_ns::new(0, 0, 0),
            ]))
        }

        DataType::Binary => Box::new(array::BinaryArray::<i32>::from_slice([[]])),
        DataType::FixedSizeBinary(size) => Box::new(array::FixedSizeBinaryArray::from_iter(
            std::iter::once(Some(vec![0; *size])),
            *size,
        )),
        DataType::LargeBinary => Box::new(array::BinaryArray::<i64>::from_slice([[]])),
        DataType::Utf8 => Box::new(array::Utf8Array::<i32>::from_slice([""])),
        DataType::LargeUtf8 => Box::new(array::Utf8Array::<i64>::from_slice([""])),
        DataType::List(field) => {
            let inner = generic_placeholder_for_datatype(field.data_type());
            let offsets = arrow2::offset::Offsets::try_from_lengths(std::iter::once(inner.len()))
                .expect("failed to create offsets buffer");
            Box::new(array::ListArray::<i32>::new(
                datatype.clone(),
                offsets.into(),
                inner,
                None,
            ))
        }

        // TODO(andreas): Unsupported type.
        // What we actually want here is an array containing a single array of size `size`.
        // But it's a bit tricky to build, because it doesn't look like we can concatenate `size` many arrays.
        DataType::FixedSizeList(_field, size) => {
            Box::new(array::FixedSizeListArray::new_null(datatype.clone(), *size))
        }

        DataType::LargeList(field) => {
            let inner = generic_placeholder_for_datatype(field.data_type());
            let offsets = arrow2::offset::Offsets::try_from_lengths(std::iter::once(inner.len()))
                .expect("failed to create offsets buffer");
            Box::new(array::ListArray::<i64>::new(
                datatype.clone(),
                offsets.into(),
                inner,
                None,
            ))
        }
        DataType::Struct(fields) => {
            let inners = fields
                .iter()
                .map(|field| generic_placeholder_for_datatype(field.data_type()));
            Box::new(array::StructArray::new(
                datatype.clone(),
                inners.collect(),
                None,
            ))
        }
        DataType::Union(fields, _types, _union_mode) => {
            if let Some(first_field) = fields.first() {
                let first_field = generic_placeholder_for_datatype(first_field.data_type());
                let first_field_len = first_field.len(); // Should be 1, but let's play this safe!
                let other_fields = fields
                    .iter()
                    .skip(1)
                    .map(|field| array::new_empty_array(field.data_type().clone()));
                let fields = std::iter::once(first_field).chain(other_fields);

                Box::new(array::UnionArray::new(
                    datatype.clone(),
                    std::iter::once(0).collect(), // Single element of type 0.
                    fields.collect(),
                    Some(std::iter::once(first_field_len as i32).collect()),
                ))
            } else {
                // Pathological case: a union with no fields can't have a placeholder with a single element?
                array::new_empty_array(datatype.clone())
            }
        }

        // TODO(andreas): Unsupported types. Fairly complex to build and we don't use it so far.
        DataType::Map(_field, _) => Box::new(array::MapArray::new_empty(datatype.clone())),

        // TODO(andreas): Unsupported type. Has only `try_new` meaning we'd have to handle all error cases.
        // But also we don't use this today anyways.
        DataType::Dictionary(_integer_type, _arc, _sorted) => {
            array::new_empty_array(datatype.clone()) // Rust type varies per integer type, use utility instead.
        }

        DataType::Decimal(_, _) => Box::new(array::Int128Array::from_slice([0])),

        DataType::Decimal256(_, _) => {
            Box::new(array::Int256Array::from_slice([types::i256::from_words(
                0, 0,
            )]))
        }

        DataType::Extension(_, datatype, _) => generic_placeholder_for_datatype(datatype),
    }
}

/// Runtime reflection about components.
pub type ComponentReflectionMap = nohash_hasher::IntMap<ComponentName, ComponentReflection>;

/// Information about a Rerun [`component`](crate::Component), generated by codegen.
#[derive(Clone, Debug)]
pub struct ComponentReflection {
    /// Markdown docstring for the component.
    pub docstring_md: &'static str,

    /// Custom placeholder value, used when not fallback was provided.
    ///
    /// This is usually the default value of the component (if any), serialized.
    ///
    /// Placeholders are useful as a base fallback value when displaying UI,
    /// especially when it's necessary to have a starting value for edit ui.
    /// Typically, this is only used when `FallbackProvider`s are not available.
    /// If there's no custom placeholder, a placeholder can be derived from the arrow datatype.
    pub custom_placeholder: Option<ArrayRef>,

    /// Datatype of the component.
    pub datatype: arrow2::datatypes::DataType,
}

/// Runtime reflection about archetypes.
pub type ArchetypeReflectionMap = nohash_hasher::IntMap<ArchetypeName, ArchetypeReflection>;

/// Utility struct containing all archetype meta information.
#[derive(Clone, Debug)]
pub struct ArchetypeReflection {
    /// The name of the field in human case.
    pub display_name: &'static str,

    /// The views that this archetype can be added to.
    ///
    /// e.g. `Spatial3DView`.
    pub view_types: &'static [&'static str],

    /// Does this have a particular scope?
    ///
    /// e.g. `"blueprint"`.
    pub scope: Option<&'static str>,

    /// All the component fields of the archetype, in the order they appear in the archetype.
    pub fields: Vec<ArchetypeFieldReflection>,
}

impl ArchetypeReflection {
    /// Iterate over this archetype's required fields.
    #[inline]
    pub fn required_fields(&self) -> impl Iterator<Item = &ArchetypeFieldReflection> {
        self.fields.iter().filter(|field| field.is_required)
    }
}

/// Additional information about an archetype's field.
#[derive(Clone, Debug)]
pub struct ArchetypeFieldReflection {
    /// The name of the field (i.e. same as `ComponentDescriptor::archetype_field_name`).
    pub name: &'static str,

    /// The name of the field in human case.
    pub display_name: &'static str,

    /// The type of the field (it's always a component).
    pub component_name: ComponentName,

    /// Markdown docstring for the field (not for the component type).
    pub docstring_md: &'static str,

    /// Is this a required component?
    pub is_required: bool,
}
