//! Reads the conventional ROS message timestamp off decoded Arrow columns.
//!
//! ROS messages conventionally carry their own acquisition time, either in a `header` or in a
//! top-level `stamp` field. [`TimestampLocation`] resolves where that sits while a decode plan is
//! built, so [`MessageDecodePlan::timestamp_nanos`] can read it straight off the decoded columns
//! afterwards without walking the schema again.

use arrow::array::{FixedSizeListArray, Int32Array, StructArray, UInt32Array};
use re_arrow_util::ArrowArrayDowncastRef as _;

use crate::message_spec::BuiltInType;

use super::decode_plan::{FieldLayout, MessageDecodePlan, MessageLayout, ValueLayout};

/// Why [`MessageDecodePlan::timestamp_nanos`] could not read a message's ROS timestamp.
#[derive(Debug, thiserror::Error)]
pub enum TimestampError {
    /// The schema declared a stamp, so the decoded columns were expected to match it.
    #[error("The decoded Arrow columns do not match the ROS {location} stamp of the schema")]
    ColumnMismatch { location: &'static str },

    /// A `sec`/`nanosec` pair that does not fit in nanoseconds since the Unix epoch.
    #[error("The ROS {location} timestamp cannot be represented in nanoseconds")]
    Unrepresentable { location: &'static str },
}

/// Pre-resolved Arrow column indexes for the conventional ROS timestamp fields.
#[derive(Debug, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub(super) enum TimestampLocation {
    None,

    /// The message has `header` field with timestamp.
    Header {
        header: usize,
        stamp: usize,
        sec: usize,
        nanosec: usize,
    },

    /// The message has a top-level `stamp` field.
    TopLevel {
        stamp: usize,
        sec: usize,
        nanosec: usize,
    },
}

impl TimestampLocation {
    /// Derives the timestamp location from a plan's message layouts, starting at `root`.
    ///
    /// If the root message declares a `header` field, only `header.stamp` is considered.
    /// A top-level `stamp` is considered only when the root has no `header` field.
    pub(super) fn from_messages(messages: &[MessageLayout], root: usize) -> Self {
        if let Some((header_index, header)) = field_by_name(messages, root, "header") {
            let ValueLayout::Message(header_id) = header.value() else {
                return Self::None;
            };
            let Some((stamp, sec, nanosec)) = stamp_indexes(messages, *header_id) else {
                return Self::None;
            };
            return Self::Header {
                header: header_index,
                stamp,
                sec,
                nanosec,
            };
        }

        let Some((stamp, sec, nanosec)) = stamp_indexes(messages, root) else {
            return Self::None;
        };
        Self::TopLevel {
            stamp,
            sec,
            nanosec,
        }
    }
}

/// Finds the field named `name` in `message_id`, along with its index.
fn field_by_name<'a>(
    messages: &'a [MessageLayout],
    message_id: usize,
    name: &str,
) -> Option<(usize, &'a FieldLayout)> {
    messages[message_id]
        .fields()
        .iter()
        .enumerate()
        .find(|(_, field)| field.name() == name)
}

/// Resolves the `stamp` field of `message_id` to its index and its `sec`/`nanosec` indexes.
///
/// `builtin_interfaces/Time` is fixed as `int32 sec` / `uint32 nanosec`, so any other shape is not
/// a stamp we can read.
fn stamp_indexes(messages: &[MessageLayout], message_id: usize) -> Option<(usize, usize, usize)> {
    let (stamp_index, stamp) = field_by_name(messages, message_id, "stamp")?;
    let ValueLayout::Message(timestamp_id) = stamp.value() else {
        return None;
    };
    let timestamp = &messages[*timestamp_id];

    let sec = timestamp.fields().iter().position(|field| {
        field.name() == "sec" && matches!(field.value(), ValueLayout::BuiltIn(BuiltInType::Int32))
    })?;
    let nanosec = timestamp.fields().iter().position(|field| {
        field.name() == "nanosec"
            && matches!(field.value(), ValueLayout::BuiltIn(BuiltInType::UInt32))
    })?;

    Some((stamp_index, sec, nanosec))
}

impl MessageDecodePlan {
    /// The ROS timestamp of every row of `messages`, in nanoseconds since the Unix epoch.
    ///
    /// `messages` is the array that [`CdrArrowDecoder::finish`](super::CdrArrowDecoder::finish)
    /// returns. `Ok(None)` means this message type carries no conventional timestamp.
    ///
    /// Timestamps are all-or-nothing: a single unreadable row fails the whole array, because a
    /// timeline that is shorter than the data cannot be indexed by row.
    pub fn timestamp_nanos(
        &self,
        messages: &FixedSizeListArray,
    ) -> Result<Option<Vec<u64>>, TimestampError> {
        let location = self.timestamp_location();
        if matches!(location, TimestampLocation::None) {
            return Ok(None);
        }

        let (secs, nanosecs) = messages
            .values()
            .downcast_array_ref::<StructArray>()
            .and_then(|root| timestamp_columns(location, root))
            .ok_or_else(|| TimestampError::ColumnMismatch {
                location: location.into(),
            })?;

        std::iter::zip(secs.iter(), nanosecs.iter())
            .map(|(sec, nanosec)| {
                let (sec, nanosec) = Option::zip(sec, nanosec)?;
                nanos_from_stamp(sec, nanosec)
            })
            .collect::<Option<Vec<u64>>>()
            .map(Some)
            .ok_or_else(|| TimestampError::Unrepresentable {
                location: location.into(),
            })
    }
}

/// Returns the timestamp columns selected by `location` from the finished message data.
fn timestamp_columns<'a>(
    location: &TimestampLocation,
    root: &'a StructArray,
) -> Option<(&'a Int32Array, &'a UInt32Array)> {
    let (timestamp_struct, sec_index, nanosec_index) = match location {
        TimestampLocation::None => return None,
        TimestampLocation::Header {
            header,
            stamp,
            sec,
            nanosec,
        } => {
            let header = root.column(*header).downcast_array_ref::<StructArray>()?;
            let stamp = header.column(*stamp).downcast_array_ref::<StructArray>()?;
            (stamp, *sec, *nanosec)
        }
        TimestampLocation::TopLevel {
            stamp,
            sec,
            nanosec,
        } => {
            let stamp = root.column(*stamp).downcast_array_ref::<StructArray>()?;
            (stamp, *sec, *nanosec)
        }
    };

    let sec = timestamp_struct
        .column(sec_index)
        .downcast_array_ref::<Int32Array>()?;
    let nanosec = timestamp_struct
        .column(nanosec_index)
        .downcast_array_ref::<UInt32Array>()?;

    Some((sec, nanosec))
}

/// Flattens a `builtin_interfaces/Time` pair into nanoseconds since the Unix epoch.
fn nanos_from_stamp(sec: i32, nanosec: u32) -> Option<u64> {
    u64::try_from(i64::from(sec) * 1_000_000_000 + i64::from(nanosec)).ok()
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;
    use crate::MessageSchema;

    #[test]
    fn top_level_stamp_is_ignored_when_header_has_no_stamp() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
test/Header header
builtin_interfaces/Time stamp

================================================================================
MSG: test/Header
int32 value

================================================================================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec
"#,
        )
        .unwrap();
        let plan = MessageDecodePlan::from_schema(&schema).unwrap();

        assert_matches!(plan.timestamp_location(), TimestampLocation::None);
    }
}
