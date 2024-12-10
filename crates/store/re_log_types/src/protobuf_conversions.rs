use std::sync::Arc;

use re_protos::TypeConversionError;

impl From<crate::EntityPath> for re_protos::common::v0::EntityPath {
    fn from(value: crate::EntityPath) -> Self {
        Self {
            path: value.to_string(),
        }
    }
}

impl TryFrom<re_protos::common::v0::EntityPath> for crate::EntityPath {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::EntityPath) -> Result<Self, Self::Error> {
        Self::parse_strict(&value.path).map_err(|err| TypeConversionError::InvalidField {
            type_name: "rerun.common.v0.EntityPath",
            field_name: "path",
            reason: err.to_string(),
        })
    }
}

impl From<re_protos::common::v0::Time> for crate::Time {
    fn from(value: re_protos::common::v0::Time) -> Self {
        Self::from_ns_since_epoch(value.nanos_since_epoch)
    }
}

impl From<crate::Time> for re_protos::common::v0::Time {
    fn from(value: crate::Time) -> Self {
        Self {
            nanos_since_epoch: value.nanos_since_epoch(),
        }
    }
}

impl From<crate::TimeInt> for re_protos::common::v0::TimeInt {
    fn from(value: crate::TimeInt) -> Self {
        Self {
            time: value.as_i64(),
        }
    }
}

impl From<re_protos::common::v0::TimeInt> for crate::TimeInt {
    fn from(value: re_protos::common::v0::TimeInt) -> Self {
        Self::new_temporal(value.time)
    }
}

impl From<crate::ResolvedTimeRange> for re_protos::common::v0::TimeRange {
    fn from(value: crate::ResolvedTimeRange) -> Self {
        Self {
            start: value.min().as_i64(),
            end: value.max().as_i64(),
        }
    }
}

impl From<re_protos::common::v0::TimeRange> for crate::ResolvedTimeRange {
    fn from(value: re_protos::common::v0::TimeRange) -> Self {
        Self::new(
            crate::TimeInt::new_temporal(value.start),
            crate::TimeInt::new_temporal(value.end),
        )
    }
}

impl From<crate::ResolvedTimeRange> for re_protos::common::v0::IndexRange {
    fn from(value: crate::ResolvedTimeRange) -> Self {
        Self {
            time_range: Some(value.into()),
        }
    }
}

impl TryFrom<re_protos::common::v0::IndexRange> for crate::ResolvedTimeRange {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::IndexRange) -> Result<Self, Self::Error> {
        value
            .time_range
            .ok_or(TypeConversionError::missing_field(
                "rerun.common.v0.IndexRange",
                "time_range",
            ))
            .map(|time_range| Self::new(time_range.start, time_range.end))
    }
}

impl TryFrom<re_protos::common::v0::IndexColumnSelector> for crate::Timeline {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::IndexColumnSelector) -> Result<Self, Self::Error> {
        let timeline_name = value
            .timeline
            .ok_or(TypeConversionError::missing_field(
                "rerun.common.v0.IndexColumnSelector",
                "timeline",
            ))?
            .name;

        // TODO(cmc): QueryExpression::filtered_index gotta be a selector
        #[allow(clippy::match_same_arms)]
        let timeline = match timeline_name.as_str() {
            "log_time" => Self::new_temporal(timeline_name),
            "log_tick" => Self::new_sequence(timeline_name),
            "frame" => Self::new_sequence(timeline_name),
            "frame_nr" => Self::new_sequence(timeline_name),
            _ => Self::new_temporal(timeline_name),
        };

        Ok(timeline)
    }
}

impl From<re_protos::common::v0::ApplicationId> for crate::ApplicationId {
    #[inline]
    fn from(value: re_protos::common::v0::ApplicationId) -> Self {
        Self(value.id)
    }
}

impl From<crate::ApplicationId> for re_protos::common::v0::ApplicationId {
    #[inline]
    fn from(value: crate::ApplicationId) -> Self {
        Self { id: value.0 }
    }
}

impl From<re_protos::common::v0::StoreKind> for crate::StoreKind {
    #[inline]
    fn from(value: re_protos::common::v0::StoreKind) -> Self {
        match value {
            re_protos::common::v0::StoreKind::Recording => Self::Recording,
            re_protos::common::v0::StoreKind::Blueprint => Self::Blueprint,
        }
    }
}

impl From<crate::StoreKind> for re_protos::common::v0::StoreKind {
    #[inline]
    fn from(value: crate::StoreKind) -> Self {
        match value {
            crate::StoreKind::Recording => Self::Recording,
            crate::StoreKind::Blueprint => Self::Blueprint,
        }
    }
}

impl From<re_protos::common::v0::StoreId> for crate::StoreId {
    #[inline]
    fn from(value: re_protos::common::v0::StoreId) -> Self {
        Self {
            kind: crate::StoreKind::Recording,
            id: Arc::new(value.id),
        }
    }
}

impl From<crate::StoreId> for re_protos::common::v0::StoreId {
    #[inline]
    fn from(value: crate::StoreId) -> Self {
        let kind: re_protos::common::v0::StoreKind = value.kind.into();
        Self {
            kind: kind as i32,
            id: String::clone(&*value.id),
        }
    }
}

impl From<re_protos::common::v0::RecordingId> for crate::StoreId {
    #[inline]
    fn from(value: re_protos::common::v0::RecordingId) -> Self {
        Self {
            kind: crate::StoreKind::Recording,
            id: Arc::new(value.id),
        }
    }
}

impl From<crate::StoreId> for re_protos::common::v0::RecordingId {
    #[inline]
    fn from(value: crate::StoreId) -> Self {
        Self {
            id: String::clone(&*value.id),
        }
    }
}
