//! Legacy types for old `MsgPack` .rrd loader

use arrow::array::RecordBatch as ArrowRecordBatch;
use re_chunk::TimePoint;
use re_log_types::{ApplicationId, StoreId};

/// Command used for activating a blueprint once it has been fully transmitted.
///
/// This command serves two purposes:
/// - It is important that a blueprint is never activated before it has been fully
///   transmitted. Displaying, or allowing a user to modify, a half-transmitted
///   blueprint can cause confusion and bad interactions with the view heuristics.
/// - Additionally, this command allows fine-tuning the activation behavior itself
///   by specifying whether the blueprint should be immediately activated, or only
///   become the default for future activations.
#[derive(Clone, Debug, PartialEq, Eq)] // `PartialEq` used for tests in another crate
#[derive(serde::Deserialize)]
pub struct LegacyBlueprintActivationCommand {
    /// The blueprint this command refers to.
    pub blueprint_id: StoreId,

    /// Immediately make this the active blueprint for the associated `app_id`.
    ///
    /// Note that setting this to `false` does not mean the blueprint may not still end
    /// up becoming active. In particular, if `make_default` is true and there is no other
    /// currently active blueprint.
    pub make_active: bool,

    /// Make this the default blueprint for the `app_id`.
    ///
    /// The default blueprint will be used as the template when the user resets the
    /// blueprint for the app. It will also become the active blueprint if no other
    /// blueprint is currently active.
    pub make_default: bool,
}

/// The most general log message sent from the SDK to the server.
#[must_use]
#[derive(Clone, Debug, PartialEq, serde::Deserialize)] // `PartialEq` used for tests in another crate
#[allow(clippy::large_enum_variant)]
// TODO(#8631): Remove `LogMsg`
pub enum LegacyLogMsg {
    /// A new recording has begun.
    ///
    /// Should usually be the first message sent.
    SetStoreInfo(LegacySetStoreInfo),

    /// Log an entity using an [`ArrowMsg`].
    //
    // TODO(#6574): the store ID should be in the metadata here so we can remove the layer on top
    ArrowMsg(StoreId, LegacyArrowMsg),

    /// Send after all messages in a blueprint to signal that the blueprint is complete.
    ///
    /// This is so that the viewer can wait with activating the blueprint until it is
    /// fully transmitted. Showing a half-transmitted blueprint can cause confusion,
    /// and also lead to problems with view heuristics.
    BlueprintActivationCommand(LegacyBlueprintActivationCommand),
}

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct LegacySetStoreInfo {
    /// A time-based UID that is only used to help keep track of when these `StoreInfo` originated
    /// and how they fit in the global ordering of events.
    //
    // NOTE: Using a raw `Tuid` instead of an actual `RowId` to prevent a nasty dependency cycle.
    // Note that both using a `RowId` as well as this whole serde/msgpack layer as a whole are hacks
    // that are destined to disappear anyhow as we are closing in on our network-exposed data APIs.
    pub row_id: LegacyTuid,

    pub info: LegacyStoreInfo,
}

// -------------------------------------------------------------

/// Message containing an Arrow payload
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct LegacyArrowMsg {
    /// Unique identifier for the chunk in this message.
    pub chunk_id: LegacyTuid,

    /// The maximum values for all timelines across the entire batch of data.
    ///
    /// Used to timestamp the batch as a whole for e.g. latency measurements without having to
    /// deserialize the arrow payload.
    pub timepoint_max: TimePoint,

    /// Schema and data for all control & data columns.
    pub batch: ArrowRecordBatch,
}

impl<'de> serde::Deserialize<'de> for LegacyArrowMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FieldVisitor;

        impl<'de> serde::de::Visitor<'de> for FieldVisitor {
            type Value = LegacyArrowMsg;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("(table_id, timepoint, buf)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                re_tracing::profile_scope!("LegacyArrowMsg::deserialize");

                let table_id: Option<LegacyTuid> = seq.next_element()?;
                let timepoint_max: Option<TimePoint> = seq.next_element()?;
                let ipc_bytes: Option<serde_bytes::ByteBuf> = seq.next_element()?;

                if let (Some(chunk_id), Some(timepoint_max), Some(buf)) =
                    (table_id, timepoint_max, ipc_bytes)
                {
                    use arrow::ipc::reader::StreamReader;

                    let stream = StreamReader::try_new(std::io::Cursor::new(buf), None)
                        .map_err(|err| serde::de::Error::custom(format!("Arrow error: {err}")))?;
                    let batches: Result<Vec<_>, _> = stream.collect();

                    let batches = batches
                        .map_err(|err| serde::de::Error::custom(format!("Arrow error: {err}")))?;

                    if batches.is_empty() {
                        return Err(serde::de::Error::custom("No RecordBatch in stream"));
                    }
                    if batches.len() > 1 {
                        return Err(serde::de::Error::custom(format!(
                            "Found {} batches in stream - expected just one.",
                            batches.len()
                        )));
                    }
                    #[allow(clippy::unwrap_used)] // is_empty check above
                    let batch = batches.into_iter().next().unwrap();

                    Ok(LegacyArrowMsg {
                        chunk_id,
                        timepoint_max,
                        batch,
                    })
                } else {
                    Err(serde::de::Error::custom(
                        "Expected (table_id, timepoint, buf)",
                    ))
                }
            }
        }

        deserializer.deserialize_tuple(3, FieldVisitor)
    }
}

// -------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
pub struct LegacyTuid {
    /// Approximate nanoseconds since epoch.
    pub time_ns: u64,

    /// Initialized to something random on each thread,
    /// then incremented for each new [`Tuid`] being allocated.
    pub inc: u64,
}

/// Information about a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct LegacyStoreInfo {
    /// The user-chosen name of the application doing the logging.
    pub application_id: ApplicationId,

    /// Should be unique for each recording.
    pub store_id: StoreId,

    /// If this store is the result of a clone, which store was it cloned from?
    ///
    /// A cloned store always gets a new unique ID.
    ///
    /// We currently only clone stores for blueprints:
    /// when we receive a _default_ blueprints on the wire (e.g. from a recording),
    /// we clone it and make the clone the _active_ blueprint.
    /// This means all active blueprints are clones.
    pub cloned_from: Option<StoreId>,

    /// True if the recording is one of the official Rerun examples.
    pub is_official_example: bool,

    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    pub started: LegacyTime,
}

#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct LegacyTime(i64);
