//! Utilities for assembling the `ros2_timestamp` timeline during schema reflection.
//!
//! ROS messages conventionally carry their own acquisition time, either in a `header` or in a
//! top-level `stamp` field. [`TimestampLocation`] resolves where that sits while a decode plan is
//! built, so the timeline can be read straight off the decoded Arrow columns afterwards.

use arrow::array::{Array as _, FixedSizeListArray, Int32Array, StructArray, UInt32Array};
use re_ros_msg::message_spec::BuiltInType;

use crate::parsers::ParserContext;

use super::decode_plan::{FieldLayout, MessageLayout, ValueLayout};

/// Pre-resolved Arrow column indexes for the conventional ROS timestamp fields.
#[derive(Debug, strum::AsRefStr)]
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

/// Returns the timestamp columns selected by `location` from the finished message data.
pub(super) fn timestamp_columns<'a>(
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
            let header = root
                .column(*header)
                .as_any()
                .downcast_ref::<StructArray>()?;
            let stamp = header
                .column(*stamp)
                .as_any()
                .downcast_ref::<StructArray>()?;
            (stamp, *sec, *nanosec)
        }
        TimestampLocation::TopLevel {
            stamp,
            sec,
            nanosec,
        } => {
            let stamp = root.column(*stamp).as_any().downcast_ref::<StructArray>()?;
            (stamp, *sec, *nanosec)
        }
    };

    let sec = timestamp_struct
        .column(sec_index)
        .as_any()
        .downcast_ref::<Int32Array>()?;
    let nanosec = timestamp_struct
        .column(nanosec_index)
        .as_any()
        .downcast_ref::<UInt32Array>()?;

    Some((sec, nanosec))
}

pub(super) fn timestamp_nanos(sec: i32, nanosec: u32) -> Option<u64> {
    u64::try_from(i64::from(sec) * 1_000_000_000 + i64::from(nanosec)).ok()
}

/// Adds one ROS timestamp timeline row for each decoded Arrow message row.
pub(super) fn add_ros2_timestamps(
    ctx: &mut ParserContext,
    location: &TimestampLocation,
    messages: &FixedSizeListArray,
) {
    if matches!(location, TimestampLocation::None) {
        // This message doesn't carry a header or top-level timestamp.
        return;
    }

    let columns = messages
        .values()
        .as_any()
        .downcast_ref::<StructArray>()
        .and_then(|root| timestamp_columns(location, root));

    let Some((secs, nanosecs)) = columns else {
        // The plan found a stamp in the schema, so the Arrow columns are expected to match it.
        re_log::warn_once!(
            "MCAP channel '{}' has a ROS 2 {} stamp that does not match its Arrow columns, so it will not appear on the `ros2_timestamp` timeline.",
            ctx.channel_topic(),
            location.as_ref()
        );
        return;
    };

    // `Chunk::from_auto_row_ids` indexes every timeline by the message's row count.
    // Drop the `ros2_timestamp` timeline in case we have a corrupt timestamp to avoid a panic there.
    let nanos: Option<Vec<u64>> = std::iter::zip(secs.iter(), nanosecs.iter())
        .map(|(sec, nanosec)| {
            let (sec, nanosec) = Option::zip(sec, nanosec)?;
            timestamp_nanos(sec, nanosec)
        })
        .collect();

    let Some(nanos) = nanos else {
        re_log::warn_once!(
            "MCAP channel '{}' has a ROS 2 {} timestamp that cannot be represented, so it will not appear on the `ros2_timestamp` timeline.",
            ctx.channel_topic(),
            location.as_ref()
        );
        return;
    };

    let time_type = ctx.time_type();
    for nanos in nanos {
        ctx.add_timestamp_cell(crate::util::TimestampCell::from_nanos_ros2(
            nanos, time_type,
        ));
    }
}
