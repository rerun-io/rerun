use prost::bytes::Bytes;

use crate::{TypeConversionError, invalid_field, missing_field};

impl From<re_log_types::StoreSource> for crate::log_msg::v1alpha1::StoreSource {
    #[inline]
    fn from(value: re_log_types::StoreSource) -> Self {
        use crate::external::prost::Message as _;

        let (kind, payload) = match value {
            re_log_types::StoreSource::Unknown => (
                crate::log_msg::v1alpha1::StoreSourceKind::Unspecified as i32,
                Bytes::new(),
            ),
            re_log_types::StoreSource::CSdk => (
                crate::log_msg::v1alpha1::StoreSourceKind::CSdk as i32,
                Bytes::new(),
            ),
            re_log_types::StoreSource::PythonSdk(python_version) => (
                crate::log_msg::v1alpha1::StoreSourceKind::PythonSdk as i32,
                crate::log_msg::v1alpha1::PythonVersion::from(python_version)
                    .encode_to_vec()
                    .into(),
            ),
            re_log_types::StoreSource::RustSdk {
                rustc_version,
                llvm_version,
            } => (
                crate::log_msg::v1alpha1::StoreSourceKind::RustSdk as i32,
                crate::log_msg::v1alpha1::CrateInfo {
                    rustc_version,
                    llvm_version,
                }
                .encode_to_vec()
                .into(),
            ),
            re_log_types::StoreSource::File { file_source } => (
                crate::log_msg::v1alpha1::StoreSourceKind::File as i32,
                crate::log_msg::v1alpha1::FileSource::from(file_source)
                    .encode_to_vec()
                    .into(),
            ),
            re_log_types::StoreSource::Viewer => (
                crate::log_msg::v1alpha1::StoreSourceKind::Viewer as i32,
                Bytes::new(),
            ),
            re_log_types::StoreSource::Other(description) => (
                crate::log_msg::v1alpha1::StoreSourceKind::Other as i32,
                description.into_bytes().into(),
            ),
        };

        Self {
            kind,
            extra: Some(crate::log_msg::v1alpha1::StoreSourceExtra { payload }),
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::StoreSource> for re_log_types::StoreSource {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::StoreSource) -> Result<Self, Self::Error> {
        use crate::external::prost::Message as _;
        use crate::log_msg::v1alpha1::StoreSourceKind;

        match value.kind() {
            StoreSourceKind::Unspecified => Ok(Self::Unknown),
            StoreSourceKind::CSdk => Ok(Self::CSdk),
            StoreSourceKind::PythonSdk => {
                let extra = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let python_version =
                    crate::log_msg::v1alpha1::PythonVersion::decode(&mut &extra.payload[..])?;
                Ok(Self::PythonSdk(re_log_types::PythonVersion::try_from(
                    python_version,
                )?))
            }
            StoreSourceKind::RustSdk => {
                let extra = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let crate_info =
                    crate::log_msg::v1alpha1::CrateInfo::decode(&mut &extra.payload[..])?;
                Ok(Self::RustSdk {
                    rustc_version: crate_info.rustc_version,
                    llvm_version: crate_info.llvm_version,
                })
            }
            StoreSourceKind::File => {
                let extra = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let file_source =
                    crate::log_msg::v1alpha1::FileSource::decode(&mut &extra.payload[..])?;
                Ok(Self::File {
                    file_source: re_log_types::FileSource::try_from(file_source)?,
                })
            }
            StoreSourceKind::Viewer => Ok(Self::Viewer),
            StoreSourceKind::Other => {
                let description = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let description =
                    String::from_utf8(description.payload.to_vec()).map_err(|err| {
                        invalid_field!(crate::log_msg::v1alpha1::StoreSource, "extra", err)
                    })?;
                Ok(Self::Other(description))
            }
        }
    }
}

impl From<re_log_types::PythonVersion> for crate::log_msg::v1alpha1::PythonVersion {
    #[inline]
    fn from(value: re_log_types::PythonVersion) -> Self {
        Self {
            major: value.major as i32,
            minor: value.minor as i32,
            patch: value.patch as i32,
            suffix: value.suffix,
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::PythonVersion> for re_log_types::PythonVersion {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::PythonVersion) -> Result<Self, Self::Error> {
        Ok(Self {
            major: value.major as u8,
            minor: value.minor as u8,
            patch: value.patch as u8,
            suffix: value.suffix,
        })
    }
}

impl From<re_log_types::FileSource> for crate::log_msg::v1alpha1::FileSource {
    #[inline]
    fn from(value: re_log_types::FileSource) -> Self {
        let kind = match value {
            re_log_types::FileSource::Cli => crate::log_msg::v1alpha1::FileSourceKind::Cli as i32,
            re_log_types::FileSource::Uri => crate::log_msg::v1alpha1::FileSourceKind::Uri as i32,
            re_log_types::FileSource::DragAndDrop { .. } => {
                crate::log_msg::v1alpha1::FileSourceKind::DragAndDrop as i32
            }
            re_log_types::FileSource::FileDialog { .. } => {
                crate::log_msg::v1alpha1::FileSourceKind::FileDialog as i32
            }
            re_log_types::FileSource::Sdk => crate::log_msg::v1alpha1::FileSourceKind::Sdk as i32,
        };

        Self { kind }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::FileSource> for re_log_types::FileSource {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::FileSource) -> Result<Self, Self::Error> {
        use crate::log_msg::v1alpha1::FileSourceKind;

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
            FileSourceKind::Unspecified => Err(invalid_field!(
                crate::log_msg::v1alpha1::FileSource,
                "kind",
                "unknown kind",
            )),
        }
    }
}

impl From<re_log_types::StoreInfo> for crate::log_msg::v1alpha1::StoreInfo {
    #[inline]
    fn from(value: re_log_types::StoreInfo) -> Self {
        Self {
            application_id: Some(value.application_id.into()),
            store_id: Some(value.store_id.into()),
            store_source: Some(value.store_source.into()),
            store_version: value
                .store_version
                .map(|v| crate::log_msg::v1alpha1::StoreVersion {
                    crate_version_bits: i32::from_le_bytes(v.to_bytes()),
                }),
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::StoreInfo> for re_log_types::StoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::StoreInfo) -> Result<Self, Self::Error> {
        let application_id: re_log_types::ApplicationId = value
            .application_id
            .ok_or(missing_field!(
                crate::log_msg::v1alpha1::StoreInfo,
                "application_id",
            ))?
            .into();
        let store_id: re_log_types::StoreId = value
            .store_id
            .ok_or(missing_field!(
                crate::log_msg::v1alpha1::StoreInfo,
                "store_id",
            ))?
            .into();
        let store_source: re_log_types::StoreSource = value
            .store_source
            .ok_or(missing_field!(
                crate::log_msg::v1alpha1::StoreInfo,
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
            store_source,
            store_version,
        })
    }
}

impl From<re_log_types::SetStoreInfo> for crate::log_msg::v1alpha1::SetStoreInfo {
    #[inline]
    fn from(value: re_log_types::SetStoreInfo) -> Self {
        Self {
            row_id: Some(value.row_id.into()),
            info: Some(value.info.into()),
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::SetStoreInfo> for re_log_types::SetStoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::SetStoreInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            row_id: value
                .row_id
                .ok_or(missing_field!(
                    crate::log_msg::v1alpha1::SetStoreInfo,
                    "row_id",
                ))?
                .try_into()?,
            info: value
                .info
                .ok_or(missing_field!(
                    crate::log_msg::v1alpha1::SetStoreInfo,
                    "info"
                ))?
                .try_into()?,
        })
    }
}

impl From<re_log_types::BlueprintActivationCommand>
    for crate::log_msg::v1alpha1::BlueprintActivationCommand
{
    #[inline]
    fn from(value: re_log_types::BlueprintActivationCommand) -> Self {
        Self {
            blueprint_id: Some(value.blueprint_id.into()),
            make_active: value.make_active,
            make_default: value.make_default,
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::BlueprintActivationCommand>
    for re_log_types::BlueprintActivationCommand
{
    type Error = TypeConversionError;

    #[inline]
    fn try_from(
        value: crate::log_msg::v1alpha1::BlueprintActivationCommand,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            blueprint_id: value
                .blueprint_id
                .ok_or(missing_field!(
                    crate::log_msg::v1alpha1::BlueprintActivationCommand,
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
    fn store_source_conversion() {
        let store_source = re_log_types::StoreSource::PythonSdk(re_log_types::PythonVersion {
            major: 3,
            minor: 8,
            patch: 0,
            suffix: "a".to_owned(),
        });
        let proto_store_source: crate::log_msg::v1alpha1::StoreSource = store_source.clone().into();
        let store_source2: re_log_types::StoreSource = proto_store_source.try_into().unwrap();
        assert_eq!(store_source, store_source2);
    }

    #[test]
    fn file_source_conversion() {
        let file_source = re_log_types::FileSource::Uri;
        let proto_file_source: crate::log_msg::v1alpha1::FileSource = file_source.clone().into();
        let file_source2: re_log_types::FileSource = proto_file_source.try_into().unwrap();
        assert_eq!(file_source, file_source2);
    }

    #[test]
    fn store_info_conversion() {
        let store_info = re_log_types::StoreInfo {
            application_id: re_log_types::ApplicationId("test".to_owned()),
            store_id: re_log_types::StoreId::from_string(
                re_log_types::StoreKind::Recording,
                "test_recording".to_owned(),
            ),
            cloned_from: None,
            store_source: re_log_types::StoreSource::PythonSdk(re_log_types::PythonVersion {
                major: 3,
                minor: 8,
                patch: 0,
                suffix: "a".to_owned(),
            }),
            store_version: None,
        };
        let proto_store_info: crate::log_msg::v1alpha1::StoreInfo = store_info.clone().into();
        let store_info2: re_log_types::StoreInfo = proto_store_info.try_into().unwrap();
        assert_eq!(store_info, store_info2);
    }

    #[test]
    fn set_store_info_conversion() {
        let set_store_info = re_log_types::SetStoreInfo {
            row_id: re_tuid::Tuid::new(),
            info: re_log_types::StoreInfo {
                application_id: re_log_types::ApplicationId("test".to_owned()),
                store_id: re_log_types::StoreId::from_string(
                    re_log_types::StoreKind::Recording,
                    "test_recording".to_owned(),
                ),
                cloned_from: None,
                store_source: re_log_types::StoreSource::PythonSdk(re_log_types::PythonVersion {
                    major: 3,
                    minor: 8,
                    patch: 0,
                    suffix: "a".to_owned(),
                }),
                store_version: None,
            },
        };
        let proto_set_store_info: crate::log_msg::v1alpha1::SetStoreInfo =
            set_store_info.clone().into();
        let set_store_info2: re_log_types::SetStoreInfo = proto_set_store_info.try_into().unwrap();
        assert_eq!(set_store_info, set_store_info2);
    }

    #[test]
    fn blueprint_activation_command_conversion() {
        let blueprint_activation_command = re_log_types::BlueprintActivationCommand {
            blueprint_id: re_log_types::StoreId::from_string(
                re_log_types::StoreKind::Blueprint,
                "test".to_owned(),
            ),
            make_active: true,
            make_default: false,
        };
        let proto_blueprint_activation_command: crate::log_msg::v1alpha1::BlueprintActivationCommand =
            blueprint_activation_command.clone().into();
        let blueprint_activation_command2: re_log_types::BlueprintActivationCommand =
            proto_blueprint_activation_command.try_into().unwrap();
        assert_eq!(blueprint_activation_command, blueprint_activation_command2);
    }
}
