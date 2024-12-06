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

impl From<crate::StoreSource> for re_protos::log_msg::v0::StoreSource {
    #[inline]
    fn from(value: crate::StoreSource) -> Self {
        use re_protos::external::prost::Message as _;

        let (kind, payload) = match value {
            crate::StoreSource::Unknown => (
                re_protos::log_msg::v0::StoreSourceKind::UnknownKind as i32,
                Vec::new(),
            ),
            crate::StoreSource::CSdk => (
                re_protos::log_msg::v0::StoreSourceKind::CSdk as i32,
                Vec::new(),
            ),
            crate::StoreSource::PythonSdk(python_version) => (
                re_protos::log_msg::v0::StoreSourceKind::PythonSdk as i32,
                re_protos::log_msg::v0::PythonVersion::from(python_version).encode_to_vec(),
            ),
            crate::StoreSource::RustSdk {
                rustc_version,
                llvm_version,
            } => (
                re_protos::log_msg::v0::StoreSourceKind::RustSdk as i32,
                re_protos::log_msg::v0::CrateInfo {
                    rustc_version,
                    llvm_version,
                }
                .encode_to_vec(),
            ),
            crate::StoreSource::File { file_source } => (
                re_protos::log_msg::v0::StoreSourceKind::File as i32,
                re_protos::log_msg::v0::FileSource::from(file_source).encode_to_vec(),
            ),
            crate::StoreSource::Viewer => (
                re_protos::log_msg::v0::StoreSourceKind::Viewer as i32,
                Vec::new(),
            ),
            crate::StoreSource::Other(description) => (
                re_protos::log_msg::v0::StoreSourceKind::Other as i32,
                description.into_bytes(),
            ),
        };

        Self {
            kind,
            extra: Some(re_protos::log_msg::v0::StoreSourceExtra { payload }),
        }
    }
}

impl TryFrom<re_protos::log_msg::v0::StoreSource> for crate::StoreSource {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::StoreSource) -> Result<Self, Self::Error> {
        use re_protos::external::prost::Message as _;
        use re_protos::log_msg::v0::StoreSourceKind;

        match value.kind() {
            StoreSourceKind::UnknownKind => Ok(Self::Unknown),
            StoreSourceKind::CSdk => Ok(Self::CSdk),
            StoreSourceKind::PythonSdk => {
                let extra = value.extra.ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.StoreSource",
                    "extra",
                ))?;
                let python_version =
                    re_protos::log_msg::v0::PythonVersion::decode(&mut &extra.payload[..])?;
                Ok(Self::PythonSdk(crate::PythonVersion::try_from(
                    python_version,
                )?))
            }
            StoreSourceKind::RustSdk => {
                let extra = value.extra.ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.StoreSource",
                    "extra",
                ))?;
                let crate_info =
                    re_protos::log_msg::v0::CrateInfo::decode(&mut &extra.payload[..])?;
                Ok(Self::RustSdk {
                    rustc_version: crate_info.rustc_version,
                    llvm_version: crate_info.llvm_version,
                })
            }
            StoreSourceKind::File => {
                let extra = value.extra.ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.StoreSource",
                    "extra",
                ))?;
                let file_source =
                    re_protos::log_msg::v0::FileSource::decode(&mut &extra.payload[..])?;
                Ok(Self::File {
                    file_source: crate::FileSource::try_from(file_source)?,
                })
            }
            StoreSourceKind::Viewer => Ok(Self::Viewer),
            StoreSourceKind::Other => {
                let description = value.extra.ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.StoreSource",
                    "extra",
                ))?;
                let description = String::from_utf8(description.payload).map_err(|err| {
                    TypeConversionError::InvalidField {
                        type_name: "rerun.log_msg.v0.StoreSource",
                        field_name: "extra",
                        reason: err.to_string(),
                    }
                })?;
                Ok(Self::Other(description))
            }
        }
    }
}

impl From<crate::PythonVersion> for re_protos::log_msg::v0::PythonVersion {
    #[inline]
    fn from(value: crate::PythonVersion) -> Self {
        let mut version = Vec::new();
        version.push(value.major);
        version.push(value.minor);
        version.push(value.patch);
        version.extend_from_slice(value.suffix.as_bytes());

        Self { version }
    }
}

impl TryFrom<re_protos::log_msg::v0::PythonVersion> for crate::PythonVersion {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::PythonVersion) -> Result<Self, Self::Error> {
        if value.version.len() < 3 {
            return Err(TypeConversionError::InvalidField {
                type_name: "rerun.log_msg.v0.PythonVersion",
                field_name: "version",
                reason: "expected at least 3 bytes".to_owned(),
            });
        }

        let major = value.version[0];
        let minor = value.version[1];
        let patch = value.version[2];
        let suffix = std::str::from_utf8(&value.version[3..])
            .map_err(|err| TypeConversionError::InvalidField {
                type_name: "rerun.log_msg.v0.PythonVersion",
                field_name: "version",
                reason: err.to_string(),
            })?
            .to_owned();

        Ok(Self {
            major,
            minor,
            patch,
            suffix,
        })
    }
}

impl From<crate::FileSource> for re_protos::log_msg::v0::FileSource {
    #[inline]
    fn from(value: crate::FileSource) -> Self {
        let kind = match value {
            crate::FileSource::Cli => re_protos::log_msg::v0::FileSourceKind::Cli as i32,
            crate::FileSource::Uri => re_protos::log_msg::v0::FileSourceKind::Uri as i32,
            crate::FileSource::DragAndDrop { .. } => {
                re_protos::log_msg::v0::FileSourceKind::DragAndDrop as i32
            }
            crate::FileSource::FileDialog { .. } => {
                re_protos::log_msg::v0::FileSourceKind::FileDialog as i32
            }
            crate::FileSource::Sdk => re_protos::log_msg::v0::FileSourceKind::Sdk as i32,
        };

        Self { kind }
    }
}

impl TryFrom<re_protos::log_msg::v0::FileSource> for crate::FileSource {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::FileSource) -> Result<Self, Self::Error> {
        use re_protos::log_msg::v0::FileSourceKind;

        match value.kind() {
            FileSourceKind::Cli => Ok(Self::Cli),
            FileSourceKind::Uri => Ok(Self::Uri),
            FileSourceKind::DragAndDrop => Ok(Self::DragAndDrop {
                recommended_application_id: None,
                recommended_recording_id: None,
                force_store_info: false,
            }),
            FileSourceKind::FileDialog => Ok(Self::FileDialog {
                recommended_application_id: None,
                recommended_recording_id: None,
                force_store_info: false,
            }),
            FileSourceKind::Sdk => Ok(Self::Sdk),
            FileSourceKind::UnknownSource => Err(TypeConversionError::InvalidField {
                type_name: "rerun.log_msg.v0.FileSource",
                field_name: "kind",
                reason: "unknown kind".to_owned(),
            }),
        }
    }
}

impl From<crate::StoreInfo> for re_protos::log_msg::v0::StoreInfo {
    #[inline]
    fn from(value: crate::StoreInfo) -> Self {
        Self {
            application_id: Some(value.application_id.into()),
            store_id: Some(value.store_id.into()),
            is_official_example: value.is_official_example,
            started: Some(value.started.into()),
            store_source: Some(value.store_source.into()),
        }
    }
}

impl TryFrom<re_protos::log_msg::v0::StoreInfo> for crate::StoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::StoreInfo) -> Result<Self, Self::Error> {
        let application_id: crate::ApplicationId = value
            .application_id
            .ok_or(TypeConversionError::missing_field(
                "rerun.log_msg.v0.StoreInfo",
                "application_id",
            ))?
            .into();
        let store_id: crate::StoreId = value
            .store_id
            .ok_or(TypeConversionError::missing_field(
                "rerun.log_msg.v0.StoreInfo",
                "store_id",
            ))?
            .into();
        let is_official_example = value.is_official_example;
        let started: crate::Time = value
            .started
            .ok_or(TypeConversionError::missing_field(
                "rerun.log_msg.v0.StoreInfo",
                "started",
            ))?
            .into();
        let store_source: crate::StoreSource = value
            .store_source
            .ok_or(TypeConversionError::missing_field(
                "rerun.log_msg.v0.StoreInfo",
                "store_source",
            ))?
            .try_into()?;

        Ok(Self {
            application_id,
            store_id,
            cloned_from: None,
            is_official_example,
            started,
            store_source,
            store_version: Some(re_build_info::CrateVersion::LOCAL),
        })
    }
}

impl From<crate::SetStoreInfo> for re_protos::log_msg::v0::SetStoreInfo {
    #[inline]
    fn from(value: crate::SetStoreInfo) -> Self {
        Self {
            row_id: Some(value.row_id.into()),
            info: Some(value.info.into()),
        }
    }
}

impl TryFrom<re_protos::log_msg::v0::SetStoreInfo> for crate::SetStoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::SetStoreInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            row_id: value
                .row_id
                .ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.SetStoreInfo",
                    "row_id",
                ))?
                .into(),
            info: value
                .info
                .ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.SetStoreInfo",
                    "info",
                ))?
                .try_into()?,
        })
    }
}

impl From<crate::BlueprintActivationCommand>
    for re_protos::log_msg::v0::BlueprintActivationCommand
{
    #[inline]
    fn from(value: crate::BlueprintActivationCommand) -> Self {
        Self {
            blueprint_id: Some(value.blueprint_id.into()),
            make_active: value.make_active,
            make_default: value.make_default,
        }
    }
}

impl TryFrom<re_protos::log_msg::v0::BlueprintActivationCommand>
    for crate::BlueprintActivationCommand
{
    type Error = TypeConversionError;

    #[inline]
    fn try_from(
        value: re_protos::log_msg::v0::BlueprintActivationCommand,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            blueprint_id: value
                .blueprint_id
                .ok_or(TypeConversionError::missing_field(
                    "rerun.log_msg.v0.BlueprintActivationCommand",
                    "blueprint_id",
                ))?
                .into(),
            make_active: value.make_active,
            make_default: value.make_default,
        })
    }
}
