use re_log_types::{EntityPath, Timeline, TimelineName};
use re_types_core::{ArchetypeName, ComponentDescriptor};

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
    /// Select the Row Id column (there can be only one)
    RowId,

    /// Select a specific time column
    Time(TimeColumnSelector),

    /// Select some component column
    Component(ComponentColumnSelector),
}

impl From<ColumnDescriptor> for ColumnSelector {
    #[inline]
    fn from(desc: ColumnDescriptor) -> Self {
        match desc {
            ColumnDescriptor::RowId(_) => Self::RowId,
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

impl From<ComponentColumnDescriptor> for ComponentColumnSelector {
    #[inline]
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self {
            entity_path: desc.entity_path,
            archetype_name: desc.archetype_name,
            archetype_field_name: desc.archetype_field_name.to_string(),
        }
    }
}

/// Select a component based on its [`ArchetypeName`] and archetype field.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentColumnSelector {
    /// The path of the entity.
    pub entity_path: EntityPath,

    /// Name of the `Archetype` associated with this data.
    // TODO(#10065): This should be the unqualified (short) archetype name in the future.
    pub archetype_name: Option<ArchetypeName>,

    /// The field within the `Archetype` associated with this data.
    pub archetype_field_name: String,
}

impl ComponentColumnSelector {
    pub fn from_descriptor(entity_path: EntityPath, descr: &ComponentDescriptor) -> Self {
        Self {
            entity_path,
            archetype_name: descr.archetype_name,
            archetype_field_name: descr.archetype_field_name.to_string(),
        }
    }

    // TODO(#10065): This is mainly used as a stop-gap until blueprints are updated to new string representation.
    pub fn try_new_from_column_name(
        entity_path: &EntityPath,
        column_name: &str,
    ) -> Result<Self, ColumnSelectorParseError> {
        let combined = format!("{entity_path}:{column_name}");
        <Self as std::str::FromStr>::from_str(&combined)
    }

    pub fn qualified_archetype_field_name(&self) -> String {
        let Self {
            archetype_name,
            archetype_field_name,
            ..
        } = self;

        match archetype_name {
            Some(archetype_name) => {
                format!("{archetype_name}:{archetype_field_name}")
            }
            None => archetype_field_name.to_string(),
        }
    }

    pub fn column_name(&self) -> String {
        // Each column can be of a different entity
        match self.archetype_name {
            Some(archetype_name) => format!(
                "{}:{}:{}",
                self.entity_path,
                // Note that we have deliberately decided to use short names here.
                archetype_name.short_name(),
                self.archetype_field_name
            ),
            None => format!("{}:{}", self.entity_path, self.archetype_field_name),
        }
    }
}

impl std::str::FromStr for ComponentColumnSelector {
    type Err = ColumnSelectorParseError;

    /// Parses a string in the form of `entity_path:archetype_field_name`.
    fn from_str(selector: &str) -> Result<Self, Self::Err> {
        if selector.is_empty() {
            return Err(ColumnSelectorParseError::EmptyString);
        }

        let s = selector;

        match (s.find(':'), s.rfind(':')) {
            (Some(first), Some(last)) if first != last => Ok(Self {
                entity_path: s[..first].into(),
                archetype_name: Some(s[first + 1..last].into()),
                archetype_field_name: s[last + 1..].into(),
            }),
            (Some(first), Some(last)) => Ok(Self {
                entity_path: s[..first].into(),
                archetype_name: None,
                archetype_field_name: s[last + 1..].into(),
            }),
            _ => Err(ColumnSelectorParseError::FormatError(selector.to_owned())),
        }
    }
}

impl std::fmt::Display for ComponentColumnSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            archetype_name,
            archetype_field_name,
        } = self;

        match archetype_name {
            Some(archetype_name) => f.write_fmt(format_args!(
                "{entity_path}:{archetype_name}:{archetype_field_name}"
            )),
            None => f.write_fmt(format_args!("{entity_path}:{archetype_field_name}")),
        }
    }
}

#[test]
fn parse_component_column_selector() {
    let column_name = "/entity_path:Test:abc";
    assert_eq!(
        column_name.parse(),
        Ok(ComponentColumnSelector {
            entity_path: "entity_path".into(),
            archetype_name: Some("Test".into()),
            archetype_field_name: "abc".into(),
        })
    );

    let column_name = "/entity_path:TestNamespace:Test:abc";
    assert_eq!(
        column_name.parse(),
        Ok(ComponentColumnSelector {
            entity_path: "entity_path".into(),
            archetype_name: Some("TestNamespace:Test".into()),
            archetype_field_name: "abc".into(),
        })
    );

    let column_name = "/entity_path:TestNamespace.Test:abc";
    assert_eq!(
        column_name.parse(),
        Ok(ComponentColumnSelector {
            entity_path: "entity_path".into(),
            archetype_name: Some("TestNamespace.Test".into()),
            archetype_field_name: "abc".into(),
        })
    );

    let column_name = "/entity_path:abc";
    assert_eq!(
        column_name.parse(),
        Ok(ComponentColumnSelector {
            entity_path: "entity_path".into(),
            archetype_name: None,
            archetype_field_name: "abc".into(),
        })
    );

    let column_name = "/:abc";
    assert_eq!(
        column_name.parse(),
        Ok(ComponentColumnSelector {
            entity_path: EntityPath::root(),
            archetype_name: None,
            archetype_field_name: "abc".into(),
        })
    );
}

#[test]
fn parse_component_column_selector_failures() {
    let column_name = "";
    assert!(matches!(
        column_name.parse::<ComponentColumnSelector>(),
        Err(ColumnSelectorParseError::EmptyString)
    ));

    let column_name = "/entity_path";
    assert!(matches!(
        column_name.parse::<ComponentColumnSelector>(),
        Err(ColumnSelectorParseError::FormatError(..))
    ));
}
