use std::sync::Arc;

use arrow::{
    array::{ArrayRef, RecordBatch, StringArray, TimestampNanosecondArray},
    datatypes::{DataType, Field, Schema, TimeUnit},
    error::ArrowError,
};

use super::rerun_manifest_registry_v1alpha1::VectorDistanceMetric;
use crate::common::v1alpha1::ComponentDescriptor;
use crate::manifest_registry::v1alpha1::{
    CreatePartitionManifestsResponse, DataSourceKind, GetDatasetSchemaResponse,
    RegisterWithDatasetResponse,
};
use crate::{invalid_field, missing_field, TypeConversionError};
use re_chunk::TimelineName;
use re_log_types::EntityPath;
use re_sorbet::ComponentColumnDescriptor;

// --- QueryDataset ---

#[derive(Debug, Clone)]
pub struct QueryDatasetRequest {
    pub entry: crate::common::v1alpha1::ext::DatasetHandle,
    pub partition_ids: Vec<crate::common::v1alpha1::ext::PartitionId>,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub scan_parameters: Option<crate::common::v1alpha1::ext::ScanParameters>,
    pub query: Option<Query>,
}

impl TryFrom<crate::manifest_registry::v1alpha1::QueryDatasetRequest> for QueryDatasetRequest {
    type Error = tonic::Status;

    fn try_from(
        value: crate::manifest_registry::v1alpha1::QueryDatasetRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            entry: value
                .entry
                .ok_or_else(|| tonic::Status::invalid_argument("entry is required"))?
                .try_into()?,

            partition_ids: value
                .partition_ids
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,

            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|tuid| {
                    let id: re_tuid::Tuid = tuid.try_into()?;
                    Ok::<_, tonic::Status>(re_chunk::ChunkId::from_u128(id.as_u128()))
                })
                .collect::<Result<Vec<_>, _>>()?,

            entity_paths: value
                .entity_paths
                .into_iter()
                .map(|path| {
                    path.try_into().map_err(|err| {
                        tonic::Status::invalid_argument(format!("invalid entity path: {err}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,

            scan_parameters: value
                .scan_parameters
                .map(|params| params.try_into())
                .transpose()?,

            query: value.query.map(|q| q.try_into()).transpose()?,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Query {
    pub latest_at: Option<QueryLatestAt>,
    pub range: Option<QueryRange>,
    pub columns_always_include_everything: bool,
    pub columns_always_include_chunk_ids: bool,
    pub columns_always_include_byte_offsets: bool,
    pub columns_always_include_entity_paths: bool,
    pub columns_always_include_static_indexes: bool,
    pub columns_always_include_global_indexes: bool,
    pub columns_always_include_component_indexes: bool,
}

impl TryFrom<crate::manifest_registry::v1alpha1::Query> for Query {
    type Error = tonic::Status;

    fn try_from(value: crate::manifest_registry::v1alpha1::Query) -> Result<Self, Self::Error> {
        let latest_at = value
            .latest_at
            .map(|latest_at| {
                Ok::<QueryLatestAt, tonic::Status>(QueryLatestAt {
                    index: latest_at
                        .index
                        .and_then(|index| index.timeline.map(|timeline| timeline.name))
                        .ok_or_else(|| {
                            tonic::Status::invalid_argument("index is required for latest_at query")
                        })?,
                    at: latest_at
                        .at
                        .ok_or_else(|| tonic::Status::invalid_argument("at is required"))?,
                    fuzzy_descriptors: latest_at
                        // TODO(cmc): I shall bring that back into a more structured form later.
                        // .into_iter()
                        // .map(|desc| FuzzyComponentDescriptor {
                        //     archetype_name: desc.archetype_name.map(Into::into),
                        //     archetype_field_name: desc.archetype_field_name.map(Into::into),
                        //     component_name: desc.component_name.map(Into::into),
                        // })
                        // .collect(),
                        .fuzzy_descriptors,
                })
            })
            .transpose()?;

        let range = value
            .range
            .map(|range| {
                Ok::<QueryRange, tonic::Status>(QueryRange {
                    index_range: range
                        .index_range
                        .ok_or_else(|| {
                            tonic::Status::invalid_argument(
                                "index_range is required for range query",
                            )
                        })?
                        .into(),
                    index: range
                        .index
                        .and_then(|index| index.timeline.map(|timeline| timeline.name))
                        .ok_or_else(|| {
                            tonic::Status::invalid_argument("index is required for range query")
                        })?,
                    fuzzy_descriptors: range
                        // TODO(cmc): I shall bring that back into a more structured form later.
                        // .into_iter()
                        // .map(|desc| FuzzyComponentDescriptor {
                        //     archetype_name: desc.archetype_name.map(Into::into),
                        //     archetype_field_name: desc.archetype_field_name.map(Into::into),
                        //     component_name: desc.component_name.map(Into::into),
                        // })
                        // .collect(),
                        .fuzzy_descriptors,
                })
            })
            .transpose()?;

        Ok(Self {
            latest_at,
            range,
            columns_always_include_byte_offsets: value.columns_always_include_byte_offsets,
            columns_always_include_chunk_ids: value.columns_always_include_chunk_ids,
            columns_always_include_component_indexes: value
                .columns_always_include_component_indexes,
            columns_always_include_entity_paths: value.columns_always_include_entity_paths,
            columns_always_include_everything: value.columns_always_include_everything,
            columns_always_include_global_indexes: value.columns_always_include_global_indexes,
            columns_always_include_static_indexes: value.columns_always_include_static_indexes,
        })
    }
}

impl From<Query> for crate::manifest_registry::v1alpha1::Query {
    fn from(value: Query) -> Self {
        crate::manifest_registry::v1alpha1::Query {
            latest_at: value.latest_at.map(|latest_at| {
                crate::manifest_registry::v1alpha1::QueryLatestAt {
                    index: Some({
                        let timeline: TimelineName = latest_at.index.into();
                        timeline.into()
                    }),
                    at: Some(latest_at.at),
                    fuzzy_descriptors: latest_at.fuzzy_descriptors,
                }
            }),
            range: value
                .range
                .map(|range| crate::manifest_registry::v1alpha1::QueryRange {
                    index: Some({
                        let timeline: TimelineName = range.index.into();
                        timeline.into()
                    }),
                    index_range: Some(range.index_range.into()),
                    fuzzy_descriptors: range.fuzzy_descriptors,
                }),
            columns_always_include_byte_offsets: value.columns_always_include_byte_offsets,
            columns_always_include_chunk_ids: value.columns_always_include_chunk_ids,
            columns_always_include_component_indexes: value
                .columns_always_include_component_indexes,
            columns_always_include_entity_paths: value.columns_always_include_entity_paths,
            columns_always_include_everything: value.columns_always_include_everything,
            columns_always_include_global_indexes: value.columns_always_include_global_indexes,
            columns_always_include_static_indexes: value.columns_always_include_static_indexes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetChunksRequest {
    pub entry: crate::common::v1alpha1::ext::DatasetHandle,
    pub partition_ids: Vec<crate::common::v1alpha1::ext::PartitionId>,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub query: Option<Query>,
}

impl TryFrom<crate::manifest_registry::v1alpha1::GetChunksRequest> for GetChunksRequest {
    type Error = tonic::Status;

    fn try_from(
        value: crate::manifest_registry::v1alpha1::GetChunksRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            entry: value
                .entry
                .ok_or_else(|| tonic::Status::invalid_argument("entry is required"))?
                .try_into()?,

            partition_ids: value
                .partition_ids
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,

            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|tuid| {
                    let id: re_tuid::Tuid = tuid.try_into()?;
                    Ok::<_, tonic::Status>(re_chunk::ChunkId::from_u128(id.as_u128()))
                })
                .collect::<Result<Vec<_>, _>>()?,

            entity_paths: value
                .entity_paths
                .into_iter()
                .map(|path| {
                    path.try_into().map_err(|err| {
                        tonic::Status::invalid_argument(format!("invalid entity path: {err}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,

            query: value.query.map(|q| q.try_into()).transpose()?,
        })
    }
}

/// A `ComponentDescriptor` meant for querying: all fields are optional.
///
/// This acts as a pattern matcher.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FuzzyComponentDescriptor {
    pub archetype_name: Option<re_chunk::ArchetypeName>,
    pub archetype_field_name: Option<re_chunk::ArchetypeFieldName>,
    pub component_name: Option<re_chunk::ComponentName>,
}

#[derive(Debug, Clone)]
pub struct QueryLatestAt {
    pub index: String,
    pub at: i64,
    pub fuzzy_descriptors: Vec<String>,
    // TODO(cmc): I shall bring that back into a more structured form later.
    // pub fuzzy_descriptors: Vec<FuzzyComponentDescriptor>,
}

#[derive(Debug, Clone)]
pub struct QueryRange {
    pub index: String,
    pub index_range: re_log_types::ResolvedTimeRange,
    pub fuzzy_descriptors: Vec<String>,
    // TODO(cmc): I shall bring that back into a more structured form later.
    // pub fuzzy_descriptors: Vec<FuzzyComponentDescriptor>,
}

// --- CreatePartitionManifestsResponse ---

impl CreatePartitionManifestsResponse {
    pub const FIELD_ID: &str = "id";
    pub const FIELD_UPDATED_AT: &str = "updated_at";
    pub const FIELD_URL: &str = "url";
    pub const FIELD_ERROR: &str = "error";

    /// The Arrow schema of the dataframe in [`Self::data`].
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::FIELD_ID, DataType::Utf8, false),
            Field::new(
                Self::FIELD_UPDATED_AT,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::FIELD_URL, DataType::Utf8, true),
            Field::new(Self::FIELD_ERROR, DataType::Utf8, true),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        updated_ats: Vec<Option<jiff::Timestamp>>,
        partition_manifest_urls: Vec<Option<String>>,
        errors: Vec<Option<String>>,
    ) -> arrow::error::Result<RecordBatch> {
        let updated_ats = updated_ats
            .into_iter()
            .map(|ts| ts.map(|ts| ts.as_nanosecond() as i64)) // ~300 years should be fine
            .collect::<Vec<_>>();

        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(TimestampNanosecondArray::from(updated_ats)),
            Arc::new(StringArray::from(partition_manifest_urls)),
            Arc::new(StringArray::from(errors)),
        ];

        RecordBatch::try_new(schema, columns)
    }
}

// TODO(#9430): I'd love if I could do this, but this creates a nasty circular dep with `re_log_encoding`.
#[cfg(all(unix, windows))] // always statically false
impl TryFrom<RecordBatch> for CreatePartitionManifestsResponse {
    type Error = tonic::Status;

    fn try_from(batch: RecordBatch) -> Result<Self, Self::Error> {
        if !Self::schema().contains(batch.schema()) {
            let typ = std::any::type_name::<Self>();
            return Err(tonic::Status::internal(format!(
                "invalid schema for {typ}: expected {:?} but got {:?}",
                Self::schema(),
                batch.schema(),
            )));
        }

        use re_log_encoding::codec::wire::encoder::Encode as _;
        batch
            .encode()
            .map(|data| Self { data: Some(data) })
            .map_err(|err| tonic::Status::internal(format!("failed to encode chunk: {err}")))?;
    }
}

// TODO(#9430): the other way around would be nice too, but same problem.

// --- GetDatasetSchemaResponse ---

#[derive(Debug, thiserror::Error)]
pub enum GetDatasetSchemaResponseError {
    #[error(transparent)]
    ArrowError(#[from] ArrowError),

    #[error(transparent)]
    TypeConversionError(#[from] TypeConversionError),
}

impl GetDatasetSchemaResponse {
    pub fn schema(self) -> Result<Schema, GetDatasetSchemaResponseError> {
        Ok(self
            .schema
            .ok_or_else(|| {
                TypeConversionError::missing_field::<GetDatasetSchemaResponse>("schema")
            })?
            .try_into()?)
    }
}

// --- RegisterWithDatasetResponse ---

impl RegisterWithDatasetResponse {
    pub const PARTITION_ID: &str = "rerun_partition_id";
    pub const PARTITION_TYPE: &str = "rerun_partition_type";
    pub const STORAGE_URL: &str = "rerun_storage_url";
    pub const TASK_ID: &str = "rerun_task_id";

    /// The Arrow schema of the dataframe in [`Self::data`].
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::PARTITION_ID, DataType::Utf8, false),
            Field::new(Self::PARTITION_TYPE, DataType::Utf8, false),
            Field::new(Self::STORAGE_URL, DataType::Utf8, false),
            Field::new(Self::TASK_ID, DataType::Utf8, false),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        partition_types: Vec<String>,
        storage_urls: Vec<String>,
        task_ids: Vec<String>,
    ) -> arrow::error::Result<RecordBatch> {
        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(StringArray::from(partition_types)),
            Arc::new(StringArray::from(storage_urls)),
            Arc::new(StringArray::from(task_ids)),
        ];

        RecordBatch::try_new(schema, columns)
    }
}

// --- DataSource --

#[derive(Debug)]
pub struct DataSource {
    pub storage_url: url::Url,
    pub kind: DataSourceKind,
}

impl DataSource {
    pub fn new_rrd(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            kind: DataSourceKind::Rrd,
        })
    }
}

impl From<DataSource> for crate::manifest_registry::v1alpha1::DataSource {
    fn from(value: DataSource) -> Self {
        crate::manifest_registry::v1alpha1::DataSource {
            storage_url: Some(value.storage_url.to_string()),
            typ: value.kind as i32,
        }
    }
}

impl TryFrom<crate::manifest_registry::v1alpha1::DataSource> for DataSource {
    type Error = TypeConversionError;

    fn try_from(
        data_source: crate::manifest_registry::v1alpha1::DataSource,
    ) -> Result<Self, Self::Error> {
        let storage_url = data_source
            .storage_url
            .ok_or_else(|| {
                missing_field!(
                    crate::manifest_registry::v1alpha1::DataSource,
                    "storage_url"
                )
            })?
            .parse()?;

        let kind = DataSourceKind::try_from(data_source.typ)?;
        if kind == DataSourceKind::Unspecified {
            return Err(invalid_field!(
                crate::manifest_registry::v1alpha1::DataSource,
                "typ",
                "data source kind is unspecified"
            ));
        }

        Ok(Self { storage_url, kind })
    }
}

/// Depending on the type of index that is being created, different properties
/// can be specified. These are defined by `IndexProperties`.
#[derive(Debug, Clone)]
pub enum IndexProperties {
    Inverted {
        store_position: bool,
        base_tokenizer: String,
    },
    VectorIvfPq {
        num_partitions: usize,
        num_sub_vectors: usize,
        metric: VectorDistanceMetric,
    },
    Btree,
}

impl std::fmt::Display for IndexProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inverted {
                store_position,
                base_tokenizer,
            } => write!(
                f,
                "Inverted {{ store_position: {store_position}, base_tokenizer: {base_tokenizer} }}"
            ),
            Self::VectorIvfPq {
                num_partitions,
                num_sub_vectors,
                metric,
            } => write!(
                f,
                "VectorIvfPq {{ num_partitions: {num_partitions}, num_sub_vectors: {num_sub_vectors}, metric: {metric:?} }}"
            ),
            Self::Btree => write!(f, "Btree"),
        }
    }
}

/// Convert `IndexProperties` into its equivalent storage model
impl From<IndexProperties> for crate::manifest_registry::v1alpha1::IndexProperties {
    fn from(other: IndexProperties) -> Self {
        match other {
            IndexProperties::Btree => Self {
                props: Some(
                    crate::manifest_registry::v1alpha1::index_properties::Props::Btree(
                        super::rerun_manifest_registry_v1alpha1::BTreeIndex {},
                    ),
                ),
            },
            IndexProperties::Inverted {
                store_position,
                base_tokenizer,
            } => Self {
                props: Some(
                    crate::manifest_registry::v1alpha1::index_properties::Props::Inverted(
                        crate::manifest_registry::v1alpha1::InvertedIndex {
                            store_position: Some(store_position),
                            base_tokenizer: Some(base_tokenizer),
                        },
                    ),
                ),
            },
            IndexProperties::VectorIvfPq {
                num_partitions,
                num_sub_vectors,
                metric,
            } => Self {
                props: Some(
                    crate::manifest_registry::v1alpha1::index_properties::Props::Vector(
                        crate::manifest_registry::v1alpha1::VectorIvfPqIndex {
                            num_partitions: Some(num_partitions as u32),
                            num_sub_vectors: Some(num_sub_vectors as u32),
                            distance_metrics: metric.into(),
                        },
                    ),
                ),
            },
        }
    }
}

// ---

impl From<ComponentColumnDescriptor> for crate::manifest_registry::v1alpha1::IndexColumn {
    fn from(value: ComponentColumnDescriptor) -> Self {
        Self {
            entity_path: Some(value.entity_path.into()),

            component: Some(ComponentDescriptor {
                archetype_name: value.archetype_name.map(|n| n.full_name().to_owned()),
                archetype_field_name: value.archetype_field_name.map(|n| n.to_string()),
                component_name: Some(value.component_name.full_name().to_owned()),
            }),
        }
    }
}
