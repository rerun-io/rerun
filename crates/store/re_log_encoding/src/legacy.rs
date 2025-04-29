//! Legacy types for old `MsgPack` .rrd loader
//!
//! The types looks the same as in 0.22, to make sure their serde works the same.

use std::{collections::BTreeMap, sync::Arc};

use arrow::array::RecordBatch as ArrowRecordBatch;

use re_chunk::{TimeInt, TimelineName};
use re_log_types::{external::re_tuid::Tuid, ApplicationId, TimeCell};

// -------------------------------------------------------------

/// Command used for activating a blueprint once it has been fully transmitted.
///
/// This command serves two purposes:
/// - It is important that a blueprint is never activated before it has been fully
///   transmitted. Displaying, or allowing a user to modify, a half-transmitted
///   blueprint can cause confusion and bad interactions with the view heuristics.
/// - Additionally, this command allows fine-tuning the activation behavior itself
///   by specifying whether the blueprint should be immediately activated, or only
///   become the default for future activations.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct LegacyBlueprintActivationCommand {
    pub blueprint_id: LegacyStoreId,
    pub make_active: bool,
    pub make_default: bool,
}

impl LegacyBlueprintActivationCommand {
    fn migrate(self) -> re_log_types::BlueprintActivationCommand {
        let Self {
            blueprint_id,
            make_active,
            make_default,
        } = self;
        re_log_types::BlueprintActivationCommand {
            blueprint_id: blueprint_id.migrate(),
            make_active,
            make_default,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct LegacyStoreId {
    pub kind: LegacyStoreKind,
    pub id: Arc<String>,
}

impl LegacyStoreId {
    fn migrate(self) -> re_log_types::StoreId {
        let Self { kind, id } = self;
        re_log_types::StoreId {
            kind: match kind {
                LegacyStoreKind::Recording => re_log_types::StoreKind::Recording,
                LegacyStoreKind::Blueprint => re_log_types::StoreKind::Blueprint,
            },
            id,
        }
    }
}

#[derive(Copy, Clone, Debug, serde::Deserialize)]
pub enum LegacyStoreKind {
    /// A recording of user-data.
    Recording,

    /// Data associated with the blueprint state.
    Blueprint,
}

// -------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum LegacyLogMsg {
    SetStoreInfo(LegacySetStoreInfo),
    ArrowMsg(LegacyStoreId, LegacyArrowMsg),
    BlueprintActivationCommand(LegacyBlueprintActivationCommand),
}

impl LegacyLogMsg {
    pub fn migrate(self) -> re_log_types::LogMsg {
        match self {
            Self::SetStoreInfo(legacy_set_store_info) => {
                let LegacySetStoreInfo { row_id, info } = legacy_set_store_info;
                let LegacyStoreInfo {
                    application_id,
                    store_id,
                    cloned_from,
                } = info;

                re_log_types::LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
                    row_id: row_id.migrate(),
                    info: re_log_types::StoreInfo {
                        application_id,
                        store_id: store_id.migrate(),
                        cloned_from: cloned_from.map(|id| id.migrate()),
                        store_source: re_log_types::StoreSource::Unknown,
                        store_version: None,
                    },
                })
            }

            Self::ArrowMsg(store_id, arrow_msg) => {
                re_log_types::LogMsg::ArrowMsg(store_id.migrate(), arrow_msg.migrate())
            }

            Self::BlueprintActivationCommand(legacy_blueprint_activation_command) => {
                re_log_types::LogMsg::BlueprintActivationCommand(
                    legacy_blueprint_activation_command.migrate(),
                )
            }
        }
    }
}

// -------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, serde::Deserialize)]
pub struct LegacySetStoreInfo {
    pub row_id: LegacyTuid,

    pub info: LegacyStoreInfo,
}

// -------------------------------------------------------------

/// Message containing an Arrow payload
#[derive(Clone, Debug)]
#[must_use]
pub struct LegacyArrowMsg {
    /// Unique identifier for the chunk in this message.
    pub chunk_id: LegacyTuid,

    /// The maximum values for all timelines across the entire batch of data.
    ///
    /// Used to timestamp the batch as a whole for e.g. latency measurements without having to
    /// deserialize the arrow payload.
    pub timepoint_max: LegacyTimePoint,

    /// Schema and data for all control & data columns.
    pub batch: ArrowRecordBatch,
}

impl LegacyArrowMsg {
    fn migrate(self) -> re_log_types::ArrowMsg {
        let Self {
            chunk_id,
            timepoint_max,
            batch,
        } = self;
        re_log_types::ArrowMsg {
            chunk_id: chunk_id.migrate(),
            timepoint_max: timepoint_max.migrate(),
            batch,
            on_release: None,
        }
    }
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
                let timepoint_max: Option<LegacyTimePoint> = seq.next_element()?;
                let ipc_bytes: Option<serde_bytes::ByteBuf> = seq.next_element()?;

                if let (Some(chunk_id), Some(timepoint_max), Some(buf)) =
                    (table_id, timepoint_max, ipc_bytes)
                {
                    let batch = arrow_from_ipc(&buf)
                        .map_err(|err| serde::de::Error::custom(format!("IPC decoding: {err}")))?;

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

        deserializer
            .deserialize_tuple(3, FieldVisitor)
            .map_err(|err| serde::de::Error::custom(format!("ArrowMsg: {err}")))
    }
}

fn arrow_from_ipc(buf: &[u8]) -> Result<ArrowRecordBatch, String> {
    use arrow::ipc::reader::StreamReader;
    let stream = StreamReader::try_new(std::io::Cursor::new(buf), None)
        .map_err(|err| format!("Arrow StreamReader error: {err}"))?;
    let batches: Result<Vec<_>, _> = stream.collect();
    let batches = batches.map_err(|err| format!("Arrow error: {err}"))?;
    if batches.is_empty() {
        return Err("No RecordBatch in stream".to_owned());
    }
    if batches.len() > 1 {
        return Err(format!(
            "Found {} batches in stream - expected just one.",
            batches.len()
        ));
    }
    #[allow(clippy::unwrap_used)] // is_empty check above
    let batch = batches.into_iter().next().unwrap();
    Ok(batch)
}

// -------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize)]
pub struct LegacyTimePoint(BTreeMap<LegacyTimeline, TimeInt>);

impl LegacyTimePoint {
    fn migrate(self) -> re_chunk::TimePoint {
        self.0
            .into_iter()
            .map(|(timeline, time_int)| {
                let LegacyTimeline { name, typ } = timeline;
                let typ = match typ {
                    LegacyTimeType::Time => {
                        if name == TimelineName::log_time() {
                            re_log_types::TimeType::TimestampNs
                        } else {
                            re_log_types::TimeType::DurationNs
                        }
                    }
                    LegacyTimeType::Sequence => re_log_types::TimeType::Sequence,
                };
                (name, TimeCell::new(typ, time_int))
            })
            .collect::<BTreeMap<_, _>>()
            .into()
    }
}

// -------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub struct LegacyTimeline {
    name: TimelineName,

    typ: LegacyTimeType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub enum LegacyTimeType {
    Time,
    Sequence,
}

// -------------------------------------------------------------

#[derive(Clone, Copy, Debug, Hash, serde::Deserialize)]
pub struct LegacyTuid {
    pub time_ns: u64,
    pub inc: u64,
}

impl LegacyTuid {
    fn migrate(&self) -> Tuid {
        Tuid::from_nanos_and_inc(self.time_ns, self.inc)
    }
}

// -------------------------------------------------------------

/// Information about a recording or blueprint.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct LegacyStoreInfo {
    pub application_id: ApplicationId,
    pub store_id: LegacyStoreId,
    pub cloned_from: Option<LegacyStoreId>,
    // pub is_official_example: bool,
    // pub started: LegacyTime,
}
