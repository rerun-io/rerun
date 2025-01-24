use arrow::datatypes::{DataType as ArrowDatatype, Field as ArrowField};
use re_log_types::{Timeline, TimelineName};

/// Describes a time column, such as `log_time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeColumnDescriptor {
    /// The timeline this column is associated with.
    pub timeline: Timeline,

    /// The Arrow datatype of the column.
    pub datatype: ArrowDatatype,
}

impl PartialOrd for TimeColumnDescriptor {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeColumnDescriptor {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            timeline,
            datatype: _,
        } = self;
        timeline.cmp(&other.timeline)
    }
}

impl TimeColumnDescriptor {
    /// Used when returning a null column, i.e. when a lookup failed.
    #[inline]
    pub fn new_null(name: TimelineName) -> Self {
        Self {
            // TODO(cmc): I picked a sequence here because I have to pick something.
            // It doesn't matter, only the name will remain in the Arrow schema anyhow.
            timeline: Timeline::new_sequence(name),
            datatype: ArrowDatatype::Null,
        }
    }

    #[inline]
    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    #[inline]
    pub fn name(&self) -> &TimelineName {
        self.timeline.name()
    }

    #[inline]
    pub fn typ(&self) -> re_log_types::TimeType {
        self.timeline.typ()
    }

    #[inline]
    pub fn datatype(&self) -> &ArrowDatatype {
        &self.datatype
    }

    #[inline]
    pub fn to_arrow_field(&self) -> ArrowField {
        let Self { timeline, datatype } = self;

        let nullable = true; // Time column must be nullable since static data doesn't have a time.

        let metadata = std::iter::once(Some((
            "sorbet.index_name".to_owned(),
            timeline.name().to_string(),
        )))
        .flatten()
        .collect();

        ArrowField::new(timeline.name().to_string(), datatype.clone(), nullable)
            .with_metadata(metadata)
    }
}
