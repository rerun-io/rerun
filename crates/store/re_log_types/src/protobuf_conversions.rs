use re_protos::TypeConversionError;
use re_protos::{invalid_field, missing_field};
use std::sync::Arc;

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
        Self::parse_strict(&value.path)
            .map_err(|err| invalid_field!(re_protos::common::v0::EntityPath, "path", err))
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
            .ok_or(missing_field!(
                re_protos::common::v0::IndexRange,
                "time_range"
            ))
            .map(|time_range| Self::new(time_range.start, time_range.end))
    }
}

impl From<re_protos::common::v0::Timeline> for crate::Timeline {
    fn from(value: re_protos::common::v0::Timeline) -> Self {
        // TODO(cmc): QueryExpression::filtered_index gotta be a selector
        #[allow(clippy::match_same_arms)]
        match value.name.as_str() {
            "log_time" => Self::new_temporal(value.name),
            "log_tick" => Self::new_sequence(value.name),
            "frame" => Self::new_sequence(value.name),
            "frame_nr" => Self::new_sequence(value.name),
            _ => Self::new_temporal(value.name),
        }
    }
}

impl From<crate::Timeline> for re_protos::common::v0::Timeline {
    fn from(value: crate::Timeline) -> Self {
        Self {
            name: value.name().to_string(),
        }
    }
}

impl TryFrom<re_protos::common::v0::IndexColumnSelector> for crate::Timeline {
    type Error = TypeConversionError;

    fn try_from(value: re_protos::common::v0::IndexColumnSelector) -> Result<Self, Self::Error> {
        let timeline = value
            .timeline
            .ok_or(missing_field!(
                re_protos::common::v0::IndexColumnSelector,
                "timeline"
            ))?
            .into();

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
            kind: value.kind().into(),
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
                let extra = value
                    .extra
                    .ok_or(missing_field!(re_protos::log_msg::v0::StoreSource, "extra"))?;
                let python_version =
                    re_protos::log_msg::v0::PythonVersion::decode(&mut &extra.payload[..])?;
                Ok(Self::PythonSdk(crate::PythonVersion::try_from(
                    python_version,
                )?))
            }
            StoreSourceKind::RustSdk => {
                let extra = value
                    .extra
                    .ok_or(missing_field!(re_protos::log_msg::v0::StoreSource, "extra"))?;
                let crate_info =
                    re_protos::log_msg::v0::CrateInfo::decode(&mut &extra.payload[..])?;
                Ok(Self::RustSdk {
                    rustc_version: crate_info.rustc_version,
                    llvm_version: crate_info.llvm_version,
                })
            }
            StoreSourceKind::File => {
                let extra = value
                    .extra
                    .ok_or(missing_field!(re_protos::log_msg::v0::StoreSource, "extra"))?;
                let file_source =
                    re_protos::log_msg::v0::FileSource::decode(&mut &extra.payload[..])?;
                Ok(Self::File {
                    file_source: crate::FileSource::try_from(file_source)?,
                })
            }
            StoreSourceKind::Viewer => Ok(Self::Viewer),
            StoreSourceKind::Other => {
                let description = value
                    .extra
                    .ok_or(missing_field!(re_protos::log_msg::v0::StoreSource, "extra"))?;
                let description = String::from_utf8(description.payload).map_err(|err| {
                    invalid_field!(re_protos::log_msg::v0::StoreSource, "extra", err)
                })?;
                Ok(Self::Other(description))
            }
        }
    }
}

impl From<crate::PythonVersion> for re_protos::log_msg::v0::PythonVersion {
    #[inline]
    fn from(value: crate::PythonVersion) -> Self {
        Self {
            major: value.major as i32,
            minor: value.minor as i32,
            patch: value.patch as i32,
            suffix: value.suffix,
        }
    }
}

impl TryFrom<re_protos::log_msg::v0::PythonVersion> for crate::PythonVersion {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::PythonVersion) -> Result<Self, Self::Error> {
        Ok(Self {
            major: value.major as u8,
            minor: value.minor as u8,
            patch: value.patch as u8,
            suffix: value.suffix,
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
            FileSourceKind::UnknownSource => Err(invalid_field!(
                re_protos::log_msg::v0::FileSource,
                "kind",
                "unknown kind",
            )),
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
            store_version: value
                .store_version
                .map(|v| re_protos::log_msg::v0::StoreVersion {
                    crate_version_bits: i32::from_le_bytes(v.to_bytes()),
                }),
        }
    }
}

impl TryFrom<re_protos::log_msg::v0::StoreInfo> for crate::StoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: re_protos::log_msg::v0::StoreInfo) -> Result<Self, Self::Error> {
        let application_id: crate::ApplicationId = value
            .application_id
            .ok_or(missing_field!(
                re_protos::log_msg::v0::StoreInfo,
                "application_id",
            ))?
            .into();
        let store_id: crate::StoreId = value
            .store_id
            .ok_or(missing_field!(
                re_protos::log_msg::v0::StoreInfo,
                "store_id",
            ))?
            .into();
        let is_official_example = value.is_official_example;
        let started: crate::Time = value
            .started
            .ok_or(missing_field!(re_protos::log_msg::v0::StoreInfo, "started"))?
            .into();
        let store_source: crate::StoreSource = value
            .store_source
            .ok_or(missing_field!(
                re_protos::log_msg::v0::StoreInfo,
                "store_source",
            ))?
            .try_into()?;
        let store_version = value
            .store_version
            .map(|v| re_build_info::CrateVersion::from_bytes(v.crate_version_bits.to_le_bytes()));

        Ok(Self {
            application_id,
            store_id,
            cloned_from: None,
            is_official_example,
            started,
            store_source,
            store_version,
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
                .ok_or(missing_field!(
                    re_protos::log_msg::v0::SetStoreInfo,
                    "row_id",
                ))?
                .into(),
            info: value
                .info
                .ok_or(missing_field!(re_protos::log_msg::v0::SetStoreInfo, "info"))?
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
                .ok_or(missing_field!(
                    re_protos::log_msg::v0::BlueprintActivationCommand,
                    "blueprint_id",
                ))?
                .into(),
            make_active: value.make_active,
            make_default: value.make_default,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn entity_path_conversion() {
        let entity_path = crate::EntityPath::parse_strict("a/b/c").unwrap();
        let proto_entity_path: re_protos::common::v0::EntityPath = entity_path.clone().into();
        let entity_path2: crate::EntityPath = proto_entity_path.try_into().unwrap();
        assert_eq!(entity_path, entity_path2);
    }

    #[test]
    fn time_conversion() {
        let time = crate::Time::from_ns_since_epoch(123456789);
        let proto_time: re_protos::common::v0::Time = time.into();
        let time2: crate::Time = proto_time.into();
        assert_eq!(time, time2);
    }

    #[test]
    fn time_int_conversion() {
        let time_int = crate::TimeInt::new_temporal(123456789);
        let proto_time_int: re_protos::common::v0::TimeInt = time_int.into();
        let time_int2: crate::TimeInt = proto_time_int.into();
        assert_eq!(time_int, time_int2);
    }

    #[test]
    fn time_range_conversion() {
        let time_range = crate::ResolvedTimeRange::new(
            crate::TimeInt::new_temporal(123456789),
            crate::TimeInt::new_temporal(987654321),
        );
        let proto_time_range: re_protos::common::v0::TimeRange = time_range.into();
        let time_range2: crate::ResolvedTimeRange = proto_time_range.into();
        assert_eq!(time_range, time_range2);
    }

    #[test]
    fn index_range_conversion() {
        let time_range = crate::ResolvedTimeRange::new(
            crate::TimeInt::new_temporal(123456789),
            crate::TimeInt::new_temporal(987654321),
        );
        let proto_index_range: re_protos::common::v0::IndexRange = time_range.into();
        let time_range2: crate::ResolvedTimeRange = proto_index_range.try_into().unwrap();
        assert_eq!(time_range, time_range2);
    }

    #[test]
    fn index_column_selector_conversion() {
        let timeline = crate::Timeline::new_temporal("log_time");
        let proto_index_column_selector: re_protos::common::v0::IndexColumnSelector =
            re_protos::common::v0::IndexColumnSelector {
                timeline: Some(timeline.into()),
            };
        let timeline2: crate::Timeline = proto_index_column_selector.try_into().unwrap();
        assert_eq!(timeline, timeline2);
    }

    #[test]
    fn application_id_conversion() {
        let application_id = crate::ApplicationId("test".to_owned());
        let proto_application_id: re_protos::common::v0::ApplicationId =
            application_id.clone().into();
        let application_id2: crate::ApplicationId = proto_application_id.into();
        assert_eq!(application_id, application_id2);
    }

    #[test]
    fn store_kind_conversion() {
        let store_kind = crate::StoreKind::Recording;
        let proto_store_kind: re_protos::common::v0::StoreKind = store_kind.into();
        let store_kind2: crate::StoreKind = proto_store_kind.into();
        assert_eq!(store_kind, store_kind2);
    }

    #[test]
    fn store_id_conversion() {
        let store_id =
            crate::StoreId::from_string(crate::StoreKind::Recording, "test_recording".to_owned());
        let proto_store_id: re_protos::common::v0::StoreId = store_id.clone().into();
        let store_id2: crate::StoreId = proto_store_id.into();
        assert_eq!(store_id, store_id2);
    }

    #[test]
    fn recording_id_conversion() {
        let store_id =
            crate::StoreId::from_string(crate::StoreKind::Recording, "test_recording".to_owned());
        let proto_recording_id: re_protos::common::v0::RecordingId = store_id.clone().into();
        let store_id2: crate::StoreId = proto_recording_id.into();
        assert_eq!(store_id, store_id2);
    }

    #[test]
    fn store_source_conversion() {
        let store_source = crate::StoreSource::PythonSdk(crate::PythonVersion {
            major: 3,
            minor: 8,
            patch: 0,
            suffix: "a".to_owned(),
        });
        let proto_store_source: re_protos::log_msg::v0::StoreSource = store_source.clone().into();
        let store_source2: crate::StoreSource = proto_store_source.try_into().unwrap();
        assert_eq!(store_source, store_source2);
    }

    #[test]
    fn file_source_conversion() {
        let file_source = crate::FileSource::Uri;
        let proto_file_source: re_protos::log_msg::v0::FileSource = file_source.clone().into();
        let file_source2: crate::FileSource = proto_file_source.try_into().unwrap();
        assert_eq!(file_source, file_source2);
    }

    #[test]
    fn store_info_conversion() {
        let store_info = crate::StoreInfo {
            application_id: crate::ApplicationId("test".to_owned()),
            store_id: crate::StoreId::from_string(
                crate::StoreKind::Recording,
                "test_recording".to_owned(),
            ),
            cloned_from: None,
            is_official_example: false,
            started: crate::Time::now(),
            store_source: crate::StoreSource::PythonSdk(crate::PythonVersion {
                major: 3,
                minor: 8,
                patch: 0,
                suffix: "a".to_owned(),
            }),
            store_version: None,
        };
        let proto_store_info: re_protos::log_msg::v0::StoreInfo = store_info.clone().into();
        let store_info2: crate::StoreInfo = proto_store_info.try_into().unwrap();
        assert_eq!(store_info, store_info2);
    }

    #[test]
    fn set_store_info_conversion() {
        let set_store_info = crate::SetStoreInfo {
            row_id: re_tuid::Tuid::new(),
            info: crate::StoreInfo {
                application_id: crate::ApplicationId("test".to_owned()),
                store_id: crate::StoreId::from_string(
                    crate::StoreKind::Recording,
                    "test_recording".to_owned(),
                ),
                cloned_from: None,
                is_official_example: false,
                started: crate::Time::now(),
                store_source: crate::StoreSource::PythonSdk(crate::PythonVersion {
                    major: 3,
                    minor: 8,
                    patch: 0,
                    suffix: "a".to_owned(),
                }),
                store_version: None,
            },
        };
        let proto_set_store_info: re_protos::log_msg::v0::SetStoreInfo =
            set_store_info.clone().into();
        let set_store_info2: crate::SetStoreInfo = proto_set_store_info.try_into().unwrap();
        assert_eq!(set_store_info, set_store_info2);
    }

    #[test]
    fn blueprint_activation_command_conversion() {
        let blueprint_activation_command = crate::BlueprintActivationCommand {
            blueprint_id: crate::StoreId::from_string(
                crate::StoreKind::Blueprint,
                "test".to_owned(),
            ),
            make_active: true,
            make_default: false,
        };
        let proto_blueprint_activation_command: re_protos::log_msg::v0::BlueprintActivationCommand =
            blueprint_activation_command.clone().into();
        let blueprint_activation_command2: crate::BlueprintActivationCommand =
            proto_blueprint_activation_command.try_into().unwrap();
        assert_eq!(blueprint_activation_command, blueprint_activation_command2);
    }
}
