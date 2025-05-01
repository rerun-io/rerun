use re_log_types::{ComponentPath, EntityPath, Timeline, TimelineName};
use re_types_core::ComponentName;

use crate::{ColumnDescriptor, ComponentColumnDescriptor, IndexColumnDescriptor};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ColumnSelectorParseError {
    #[error("Expected column selector, found empty string")]
    EmptyString,

    #[error("Expected string in the form of `entity_path:component_name`, got: {0}")]
    FormatError(String),
}

/// Describes a column selection to return as part of a query.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnSelector {
    Time(TimeColumnSelector),
    Component(ComponentColumnSelector),
    //TODO(jleibs): Add support for archetype-based component selection.
    //ArchetypeField(ArchetypeFieldColumnSelector),
}

impl From<ColumnDescriptor> for ColumnSelector {
    #[inline]
    fn from(desc: ColumnDescriptor) -> Self {
        match desc {
            ColumnDescriptor::Time(desc) => Self::Time(desc.into()),
            ColumnDescriptor::Component(desc) => Self::Component(desc.into()),
        }
    }
}

impl From<TimeColumnSelector> for ColumnSelector {
    #[inline]
    fn from(desc: TimeColumnSelector) -> Self {
        Self::Time(desc)
    }
}

impl From<ComponentColumnSelector> for ColumnSelector {
    #[inline]
    fn from(desc: ComponentColumnSelector) -> Self {
        Self::Component(desc)
    }
}

/// Select a time column.
//
// TODO(cmc): This shouldn't be specific to time, this should be an `IndexColumnSelector` or smth.
// Particularly unfortunate that this one already leaks into the public API…
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeColumnSelector {
    /// The name of the timeline.
    pub timeline: TimelineName,
}

impl From<TimelineName> for TimeColumnSelector {
    #[inline]
    fn from(timeline: TimelineName) -> Self {
        Self { timeline }
    }
}

impl From<Timeline> for TimeColumnSelector {
    #[inline]
    fn from(timeline: Timeline) -> Self {
        Self {
            timeline: *timeline.name(),
        }
    }
}

impl From<&str> for TimeColumnSelector {
    #[inline]
    fn from(timeline: &str) -> Self {
        Self {
            timeline: timeline.into(),
        }
    }
}

impl From<String> for TimeColumnSelector {
    #[inline]
    fn from(timeline: String) -> Self {
        Self {
            timeline: timeline.into(),
        }
    }
}

impl From<IndexColumnDescriptor> for TimeColumnSelector {
    #[inline]
    fn from(desc: IndexColumnDescriptor) -> Self {
        Self {
            timeline: desc.timeline_name(),
        }
    }
}

/// Select a component based on its `EntityPath` and `ComponentName`.
///
/// Note, that in the future when Rerun supports duplicate tagged components
/// on the same entity, this selector may be ambiguous. In this case, the
/// query result will return an Error if it cannot determine a single selected
/// component.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentColumnSelector {
    /// The path of the entity.
    pub entity_path: EntityPath,

    /// Semantic name associated with this data.
    ///
    /// This string will be flexibly matched against the available component names.
    /// Valid matches are case invariant matches of either the full name or the short name.
    pub component_name: String,
}

impl From<ComponentColumnDescriptor> for ComponentColumnSelector {
    #[inline]
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self {
            entity_path: desc.entity_path.clone(),
            component_name: desc.component_name.short_name().to_owned(),
        }
    }
}

impl std::str::FromStr for ComponentColumnSelector {
    type Err = ColumnSelectorParseError;

    /// Parses a string in the form of `entity_path:component_name`.
    ///
    /// Note that no attempt is made to interpret `component_name`. In particular, we don't attempt
    /// to prepend a `rerun.components.` prefix like [`ComponentPath::from_str`] does.
    fn from_str(selector: &str) -> Result<Self, Self::Err> {
        if selector.is_empty() {
            return Err(ColumnSelectorParseError::EmptyString);
        }

        let tokens = re_log_types::tokenize_by(selector, b":");

        match tokens.as_slice() {
            &[entity_path_token, ":", component_name_token] => Ok(Self {
                entity_path: EntityPath::from(entity_path_token),
                component_name: component_name_token.to_owned(),
            }),

            _ => Err(ColumnSelectorParseError::FormatError(selector.to_owned())),
        }
    }
}

impl From<ComponentPath> for ComponentColumnSelector {
    #[inline]
    fn from(path: ComponentPath) -> Self {
        Self {
            entity_path: path.entity_path,
            component_name: path.component_name.as_str().to_owned(),
        }
    }
}

impl ComponentColumnSelector {
    /// Select a component of a given type, based on its  [`EntityPath`]
    #[inline]
    pub fn new<C: re_types_core::Component>(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            component_name: C::name().short_name().to_owned(),
        }
    }

    /// Select a component based on its [`EntityPath`] and [`ComponentName`].
    #[inline]
    pub fn new_for_component_name(entity_path: EntityPath, component_name: ComponentName) -> Self {
        Self {
            entity_path,
            component_name: component_name.short_name().to_owned(),
        }
    }
}

impl std::fmt::Display for ComponentColumnSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            component_name,
        } = self;

        f.write_fmt(format_args!("{entity_path}:{component_name}"))
    }
}

// TODO(jleibs): Add support for archetype-based column selection.
/*
/// Select a component based on its `Archetype` and field.
pub struct ArchetypeFieldColumnSelector {
    /// The path of the entity.
    entity_path: EntityPath,

    /// Name of the `Archetype` associated with this data.
    archetype: ArchetypeName,

    /// The field within the `Archetype` associated with this data.
    field: String,
}
*/
