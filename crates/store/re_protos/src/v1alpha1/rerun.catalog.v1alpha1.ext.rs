use crate::{
    common::v1alpha1::ext::{DatasetHandle, EntryId},
    missing_field, TypeConversionError,
};

// --- EntryDetails ---

#[derive(Debug, Clone)]
pub struct EntryDetails {
    pub id: EntryId,
    pub name: String,
    pub kind: crate::catalog::v1alpha1::EntryKind,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

impl TryFrom<crate::catalog::v1alpha1::EntryDetails> for EntryDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::catalog::v1alpha1::EntryDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(crate::catalog::v1alpha1::EntryDetails, "id"))?
                .try_into()?,
            name: value.name.ok_or(missing_field!(
                crate::catalog::v1alpha1::EntryDetails,
                "name"
            ))?,
            kind: value.entry_kind.try_into()?,
            created_at: {
                let ts = value.created_at.ok_or(missing_field!(
                    crate::catalog::v1alpha1::EntryDetails,
                    "created_at"
                ))?;
                jiff::Timestamp::new(ts.seconds, ts.nanos)?
            },
            updated_at: {
                let ts = value.updated_at.ok_or(missing_field!(
                    crate::catalog::v1alpha1::EntryDetails,
                    "updated_at"
                ))?;
                jiff::Timestamp::new(ts.seconds, ts.nanos)?
            },
        })
    }
}

impl From<EntryDetails> for crate::catalog::v1alpha1::EntryDetails {
    fn from(value: EntryDetails) -> Self {
        Self {
            id: Some(value.id.into()),
            name: Some(value.name),
            entry_kind: value.kind as _,
            created_at: {
                let ts = value.created_at;
                Some(prost_types::Timestamp {
                    seconds: ts.as_second(),
                    nanos: ts.subsec_nanosecond(),
                })
            },
            updated_at: {
                let ts = value.updated_at;
                Some(prost_types::Timestamp {
                    seconds: ts.as_second(),
                    nanos: ts.subsec_nanosecond(),
                })
            },
        }
    }
}

// --- DatasetEntry ---

#[derive(Debug, Clone)]
pub struct DatasetEntry {
    pub details: EntryDetails,
    pub handle: DatasetHandle,
}

impl TryFrom<crate::catalog::v1alpha1::DatasetEntry> for DatasetEntry {
    type Error = TypeConversionError;

    fn try_from(value: crate::catalog::v1alpha1::DatasetEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: value
                .details
                .ok_or(missing_field!(
                    crate::catalog::v1alpha1::DatasetEntry,
                    "details"
                ))?
                .try_into()?,
            handle: value
                .dataset_handle
                .ok_or(missing_field!(
                    crate::catalog::v1alpha1::DatasetEntry,
                    "handle"
                ))?
                .try_into()?,
        })
    }
}

impl From<DatasetEntry> for crate::catalog::v1alpha1::DatasetEntry {
    fn from(value: DatasetEntry) -> Self {
        Self {
            details: Some(value.details.into()),
            dataset_handle: Some(value.handle.into()),
        }
    }
}

// --- ReadDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct CreateDatasetEntryResponse {
    pub dataset: DatasetEntry,
}

impl TryFrom<crate::catalog::v1alpha1::CreateDatasetEntryResponse> for CreateDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::catalog::v1alpha1::CreateDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset: value
                .dataset
                .ok_or(missing_field!(
                    crate::catalog::v1alpha1::CreateDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}

// --- ReadDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct ReadDatasetEntryResponse {
    pub dataset_entry: DatasetEntry,
}

impl TryFrom<crate::catalog::v1alpha1::ReadDatasetEntryResponse> for ReadDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::catalog::v1alpha1::ReadDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_entry: value
                .dataset
                .ok_or(missing_field!(
                    crate::catalog::v1alpha1::ReadDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}
