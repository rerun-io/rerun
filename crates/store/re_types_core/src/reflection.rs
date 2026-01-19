//! Run-time reflection for reading meta-data about components and archetypes.

use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef};
use arrow::datatypes::TimeUnit;

use crate::{ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType};

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
    pub component_identifiers: ComponentIdentifierReflectionMap,
    pub archetypes: ArchetypeReflectionMap,
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
    datatype: &arrow::datatypes::DataType,
) -> arrow::array::ArrayRef {
    use arrow::array::{self, types};
    use arrow::datatypes::{DataType, IntervalUnit};

    match datatype {
        DataType::Null => Arc::new(array::NullArray::new(1)),
        DataType::Boolean => Arc::new(array::BooleanArray::from_iter([Some(false)])),
        DataType::Int8 => Arc::new(array::Int8Array::from_iter([0])),
        DataType::Int16 => Arc::new(array::Int16Array::from_iter([0])),
        DataType::Int32 => Arc::new(array::Int32Array::from_iter([0])),
        DataType::Int64 => Arc::new(array::Int64Array::from_iter([0])),

        DataType::Date32 => Arc::new(array::Date32Array::from_iter([Some(0)])),

        DataType::Date64 => Arc::new(array::Date64Array::from_iter([Some(0)])),

        DataType::Interval(interval_unit) => match interval_unit {
            IntervalUnit::YearMonth => {
                Arc::new(array::IntervalYearMonthArray::from_iter([Some(0)]))
            }
            IntervalUnit::DayTime => Arc::new(array::IntervalDayTimeArray::from_iter([Some(
                types::IntervalDayTime::new(0, 0),
            )])),
            IntervalUnit::MonthDayNano => {
                Arc::new(array::IntervalMonthDayNanoArray::from_iter([Some(
                    types::IntervalMonthDayNano::new(0, 0, 0),
                )]))
            }
        },

        DataType::Timestamp(time_unit, _) => match time_unit {
            TimeUnit::Second => Arc::new(array::TimestampSecondArray::new(vec![0].into(), None)),

            TimeUnit::Millisecond => {
                Arc::new(array::TimestampMillisecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Microsecond => {
                Arc::new(array::TimestampMicrosecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Nanosecond => {
                Arc::new(array::TimestampNanosecondArray::new(vec![0].into(), None))
            }
        },

        DataType::Time32(time_unit) => match time_unit {
            TimeUnit::Second => Arc::new(array::Time32SecondArray::new(vec![0].into(), None)),

            TimeUnit::Millisecond => {
                Arc::new(array::Time32MillisecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Microsecond | TimeUnit::Nanosecond => {
                re_log::debug_once!(
                    "Attempted to create a placeholder for out-of-spec datatype: {datatype}"
                );
                array::new_empty_array(datatype)
            }
        },

        DataType::Time64(time_unit) => match time_unit {
            TimeUnit::Microsecond => {
                Arc::new(array::Time64MicrosecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Nanosecond => {
                Arc::new(array::Time64NanosecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Second | TimeUnit::Millisecond => {
                re_log::debug_once!(
                    "Attempted to create a placeholder for out-of-spec datatype: {datatype}"
                );
                array::new_empty_array(datatype)
            }
        },

        DataType::Duration(time_unit) => match time_unit {
            TimeUnit::Second => Arc::new(array::DurationSecondArray::new(vec![0].into(), None)),

            TimeUnit::Millisecond => {
                Arc::new(array::DurationMillisecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Microsecond => {
                Arc::new(array::DurationMicrosecondArray::new(vec![0].into(), None))
            }

            TimeUnit::Nanosecond => {
                Arc::new(array::DurationNanosecondArray::new(vec![0].into(), None))
            }
        },

        DataType::UInt8 => Arc::new(array::UInt8Array::from_iter([0])),
        DataType::UInt16 => Arc::new(array::UInt16Array::from_iter([0])),
        DataType::UInt32 => Arc::new(array::UInt32Array::from_iter([0])),
        DataType::UInt64 => Arc::new(array::UInt64Array::from_iter([0])),
        DataType::Float16 => Arc::new(array::Float16Array::from_iter([half::f16::ZERO])),
        DataType::Float32 => Arc::new(array::Float32Array::from_iter([0.0])),
        DataType::Float64 => Arc::new(array::Float64Array::from_iter([0.0])),

        DataType::Binary => Arc::new(array::GenericBinaryArray::<i32>::from_vec(vec![&[]])),
        DataType::LargeBinary => Arc::new(array::GenericBinaryArray::<i64>::from_vec(vec![&[]])),

        DataType::Utf8 => Arc::new(array::StringArray::from(vec![""])),
        DataType::LargeUtf8 => Arc::new(array::LargeStringArray::from(vec![""])),

        DataType::List(field) => {
            let inner = generic_placeholder_for_datatype(field.data_type());
            let offsets = arrow::buffer::OffsetBuffer::from_lengths(std::iter::once(inner.len()));
            Arc::new(array::GenericListArray::<i32>::new(
                field.clone(),
                offsets,
                inner,
                None,
            ))
        }

        DataType::FixedSizeList(field, size) => {
            let size = *size as usize;
            let value_data: ArrayRef = {
                match field.data_type() {
                    DataType::Boolean => Arc::new(array::BooleanArray::from(vec![false; size])),

                    DataType::Int8 => Arc::new(array::Int8Array::from(vec![0; size])),
                    DataType::Int16 => Arc::new(array::Int16Array::from(vec![0; size])),
                    DataType::Int32 => Arc::new(array::Int32Array::from(vec![0; size])),
                    DataType::Int64 => Arc::new(array::Int64Array::from(vec![0; size])),

                    DataType::UInt8 => Arc::new(array::UInt8Array::from(vec![0; size])),
                    DataType::UInt16 => Arc::new(array::UInt16Array::from(vec![0; size])),
                    DataType::UInt32 => Arc::new(array::UInt32Array::from(vec![0; size])),
                    DataType::UInt64 => Arc::new(array::UInt64Array::from(vec![0; size])),

                    DataType::Float16 => {
                        Arc::new(array::Float16Array::from(vec![half::f16::ZERO; size]))
                    }
                    DataType::Float32 => Arc::new(array::Float32Array::from(vec![0.0; size])),
                    DataType::Float64 => Arc::new(array::Float64Array::from(vec![0.0; size])),

                    _ => {
                        // TODO(emilk)
                        re_log::debug_once!(
                            "Unimplemented: placeholder value for FixedSizeListArray of {:?}",
                            field.data_type()
                        );
                        return array::new_empty_array(datatype);
                    }
                }
            };
            if let Ok(list_data) = array::ArrayData::builder(datatype.clone())
                .len(1)
                .add_child_data(value_data.into_data())
                .build()
            {
                Arc::new(array::FixedSizeListArray::from(list_data))
            } else {
                re_log::warn_once!("Bug in FixedSizeListArray of {}", field.data_type());
                array::new_empty_array(datatype)
            }
        }

        DataType::LargeList(field) => {
            let inner = generic_placeholder_for_datatype(field.data_type());
            let offsets = arrow::buffer::OffsetBuffer::from_lengths(std::iter::once(inner.len()));
            Arc::new(array::GenericListArray::<i64>::new(
                field.clone(),
                offsets,
                inner,
                None,
            ))
        }
        DataType::Struct(fields) => {
            let inners = fields
                .iter()
                .map(|field| generic_placeholder_for_datatype(field.data_type()));
            Arc::new(array::StructArray::new(
                fields.clone(),
                inners.collect(),
                None,
            ))
        }

        DataType::Decimal32(_, _) => Arc::new(array::Decimal32Array::from_iter([0])),
        DataType::Decimal64(_, _) => Arc::new(array::Decimal64Array::from_iter([0])),
        DataType::Decimal128(_, _) => Arc::new(array::Decimal128Array::from_iter([0])),

        DataType::Decimal256(_, _) => Arc::new(array::Decimal256Array::from_iter([
            arrow::datatypes::i256::ZERO,
        ])),

        DataType::FixedSizeBinary(length) => Arc::new(array::FixedSizeBinaryArray::new(
            *length,
            vec![0u8; *length as usize].into(),
            None,
        )),

        DataType::Dictionary { .. }
        | DataType::Union { .. }
        | DataType::Map { .. }
        | DataType::BinaryView
        | DataType::Utf8View
        | DataType::ListView { .. }
        | DataType::LargeListView { .. }
        | DataType::RunEndEncoded { .. } => {
            // TODO(emilk)
            re_log::debug_once!("Unimplemented: placeholder value for: {datatype}");
            array::new_empty_array(datatype) // TODO(emilk)
        }
    }
}

/// Runtime reflection about components.
pub type ComponentReflectionMap = nohash_hasher::IntMap<ComponentType, ComponentReflection>;

/// Runtime reflection about component identifiers.
pub type ComponentIdentifierReflectionMap =
    nohash_hasher::IntMap<ComponentIdentifier, ComponentDescriptor>;

pub fn generate_component_identifier_reflection(
    archetypes: &ArchetypeReflectionMap,
) -> ComponentIdentifierReflectionMap {
    archetypes
        .iter()
        .flat_map(|(archetype_name, archetype_reflection)| {
            archetype_reflection.fields.iter().map(|field| {
                let descr = field.component_descriptor(*archetype_name);
                (descr.component, descr)
            })
        })
        .collect()
}

/// Information about a Rerun [`component`](crate::Component), generated by codegen.
#[derive(Clone, Debug)]
pub struct ComponentReflection {
    /// Markdown docstring for the component.
    pub docstring_md: &'static str,

    /// If deprecated, this explains since when, and what to use instead.
    pub deprecation_summary: Option<&'static str>,

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
    pub datatype: arrow::datatypes::DataType,

    /// Checks that the given Arrow array can be deserialized into a collection of [`Self`]s.
    pub verify_arrow_array: fn(&dyn arrow::array::Array) -> crate::DeserializationResult<()>,
}

/// Runtime reflection about archetypes.
pub type ArchetypeReflectionMap = nohash_hasher::IntMap<ArchetypeName, ArchetypeReflection>;

/// Utility struct containing all archetype meta information.
#[derive(Clone, Debug)]
pub struct ArchetypeReflection {
    /// The name of the field in human case.
    pub display_name: &'static str,

    /// If deprecated, this explains since when, and what to use instead.
    pub deprecation_summary: Option<&'static str>,

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

    pub fn get_field(&self, field_name: &str) -> Option<&ArchetypeFieldReflection> {
        self.fields.iter().find(|field| field.name == field_name)
    }
}

/// Additional information about an archetype's field.
#[derive(Clone, Debug)]
pub struct ArchetypeFieldReflection {
    /// The name of the field.
    pub name: &'static str,

    /// The name of the field in human case.
    pub display_name: &'static str,

    /// The type of the field (it's always a component).
    pub component_type: ComponentType,

    /// Markdown docstring for the field (not for the component type).
    pub docstring_md: &'static str,

    /// Is this a required component?
    pub is_required: bool,
}

impl ArchetypeFieldReflection {
    /// Returns the component descriptor for this field.
    #[inline]
    pub fn component_descriptor(&self, archetype_name: ArchetypeName) -> ComponentDescriptor {
        ComponentDescriptor {
            component_type: Some(self.component_type),
            component: self.component(archetype_name),
            archetype: Some(archetype_name),
        }
    }

    /// Returns the component identifier for this field.
    #[inline]
    pub fn component(&self, archetype_name: ArchetypeName) -> ComponentIdentifier {
        format!("{}:{}", archetype_name.short_name(), self.name).into()
    }
}

/// An extension of [`ComponentDescriptor`] that helps handling components that are
/// built into Rerun.
///
/// Note that in general, [`ComponentDescriptor::archetype`] does not place any
/// constraints on the [`ComponentDescriptor::component`] field, which can be arbitrary.
///
/// Components that are built into the Rerun viewer follow the convention that
/// the `component` field starts with the short name of its archetype.
pub trait ComponentDescriptorExt {
    /// Returns the field name of a Rerun-builtin type.
    ///
    /// This is the result of stripping the Rerun-builtin [`ArchetypeName`]
    /// from [`ComponentDescriptor::component`].
    fn archetype_field_name(&self) -> &str;

    /// Unconditionally sets [`ComponentDescriptor::archetype`] to the given one.
    ///
    /// Following the viewer's conventions, this also changes the archetype
    /// part of [`ComponentDescriptor::component`].
    fn with_builtin_archetype(self, archetype: impl Into<ArchetypeName>) -> Self;

    /// Sets [`ComponentDescriptor::archetype`] to the given one iff it's not already set.
    ///
    /// Following the viewer's conventions, this also changes the archetype
    /// part of [`ComponentDescriptor::component`].
    fn or_with_builtin_archetype(self, archetype: impl Fn() -> ArchetypeName) -> Self;
}

/// Constructs a [`ComponentIdentifier`] from this archetype by supplying a field name.
///
/// Mainly used as a convenience function to create [`ComponentDescriptor`]s for
/// Rerun-builtin types. In general, the [`ArchetypeName`] does not place any restrictions
/// on the contents of [`ComponentIdentifier`].
#[inline]
fn with_field(archetype: ArchetypeName, field_name: impl AsRef<str>) -> ComponentIdentifier {
    format!("{}:{}", archetype.short_name(), field_name.as_ref()).into()
}

impl ComponentDescriptorExt for ComponentDescriptor {
    fn archetype_field_name(&self) -> &str {
        self.archetype
            .and_then(|archetype| {
                self.component
                    .strip_prefix(&format!("{}:", archetype.short_name()))
            })
            .unwrap_or_else(|| self.component.as_str())
    }

    #[inline]
    fn with_builtin_archetype(mut self, archetype: impl Into<ArchetypeName>) -> Self {
        let archetype = archetype.into();
        {
            let field_name = self.archetype_field_name();
            self.component = with_field(archetype, field_name);
        }
        self.archetype = Some(archetype);
        self
    }

    #[inline]
    fn or_with_builtin_archetype(mut self, archetype: impl Fn() -> ArchetypeName) -> Self {
        if self.archetype.is_none() {
            let archetype = archetype();
            self.component = with_field(archetype, self.component);
            self.archetype = Some(archetype);
        }
        self
    }
}

#[cfg(test)]
mod test {
    use super::{ComponentDescriptor, ComponentDescriptorExt as _, with_field};
    use crate::ArchetypeName;

    #[test]
    fn component_descriptor_manipulation() {
        let archetype_name: ArchetypeName = "rerun.archetypes.MyExample".into();
        let descr = ComponentDescriptor {
            archetype: Some(archetype_name),
            component: with_field(archetype_name, "test"),
            component_type: Some("user.Whatever".into()),
        };
        assert_eq!(descr.archetype_field_name(), "test");
        assert_eq!(descr.display_name(), "MyExample:test");

        let archetype_name: ArchetypeName = "rerun.archetypes.MyOtherExample".into();
        let descr = descr.with_builtin_archetype(archetype_name);
        assert_eq!(descr.archetype_field_name(), "test");
        assert_eq!(descr.display_name(), "MyOtherExample:test");
    }
}
