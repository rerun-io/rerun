use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use re_log_types::{Timeline, TimelineName};

use crate::MetadataExt as _;

#[derive(thiserror::Error, Debug)]
#[error("Unsupported time type: {datatype}")]
pub struct UnsupportedTimeType {
    pub datatype: ArrowDatatype,
}

/// Describes a time column, such as `log_time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexColumnDescriptor {
    /// The timeline this column is associated with.
    pub timeline: Timeline,

    /// The Arrow datatype of the column.
    pub datatype: ArrowDatatype,

    /// Are the indices in this column sorted?
    ///
    /// `false` means either "unsorted" or "unknown".
    pub is_sorted: bool,
}

impl PartialOrd for IndexColumnDescriptor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexColumnDescriptor {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            timeline,
            datatype: _,
            is_sorted: _,
        } = self;
        timeline.cmp(&other.timeline)
    }
}

impl IndexColumnDescriptor {
    /// Used when returning a null column, i.e. when a lookup failed.
    #[inline]
    pub fn new_null(name: TimelineName) -> Self {
        Self {
            // TODO(cmc): I picked a sequence here because I have to pick something.
            // It doesn't matter, only the name will remain in the Arrow schema anyhow.
            timeline: Timeline::new_sequence(name),
            datatype: ArrowDatatype::Null,
            is_sorted: true,
        }
    }

    #[inline]
    pub fn from_timeline(timeline: Timeline, is_sorted: bool) -> Self {
        Self {
            timeline,
            datatype: timeline.datatype(),
            is_sorted,
        }
    }

    #[inline]
    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    #[inline]
    pub fn timeline_name(&self) -> TimelineName {
        *self.timeline.name()
    }

    #[inline]
    pub fn column_name(&self) -> &str {
        self.timeline.name()
    }

    #[inline]
    pub fn datatype(&self) -> &ArrowDatatype {
        &self.datatype
    }

    /// Are the indices in this column sorted?
    ///
    /// `false` means either "unsorted" or "unknown".
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.is_sorted
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self {
            timeline,
            datatype,
            is_sorted,
        } = self;

        let nullable = true; // Time column must be nullable since static data doesn't have a time.

        let mut metadata = std::collections::HashMap::from([
            (
                crate::metadata::RERUN_KIND.to_owned(),
                crate::ColumnKind::Index.to_string(),
            ),
            (
                crate::metadata::SORBET_INDEX_NAME.to_owned(),
                timeline.name().to_string(),
            ),
        ]);
        if *is_sorted {
            metadata.insert("rerun:is_sorted".to_owned(), "true".to_owned());
        }

        ArrowField::new(timeline.name().to_string(), datatype.clone(), nullable)
            .with_metadata(metadata)
    }
}

impl From<Timeline> for IndexColumnDescriptor {
    fn from(timeline: Timeline) -> Self {
        Self {
            timeline,
            datatype: timeline.datatype(),
            is_sorted: false, // assume the worst
        }
    }
}

impl TryFrom<&ArrowField> for IndexColumnDescriptor {
    type Error = UnsupportedTimeType;

    fn try_from(field: &ArrowField) -> Result<Self, Self::Error> {
        let name = if let Some(name) = field.metadata().get(crate::metadata::SORBET_INDEX_NAME) {
            name.to_owned()
        } else {
            re_log::debug_once!(
                "Timeline '{}' is missing 'rerun:index_name' metadata. Falling back on field/column name",
                field.name()
            );
            field.name().to_owned()
        };

        let datatype = field.data_type().clone();

        let Some(time_type) = re_log_types::TimeType::from_arrow_datatype(&datatype) else {
            return Err(UnsupportedTimeType { datatype });
        };

        let timeline = Timeline::new(name, time_type);

        Ok(Self {
            timeline,
            datatype,
            is_sorted: field.metadata().get_bool("rerun:is_sorted"),
        })
    }
}
