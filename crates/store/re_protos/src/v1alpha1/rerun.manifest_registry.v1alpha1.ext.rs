use std::sync::Arc;

use arrow::{
    array::{Array, ArrayRef, RecordBatch, StringArray, TimestampNanosecondArray},
    datatypes::{DataType, Field, Schema, TimeUnit},
    error::ArrowError,
};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::TimelineName;
use re_log_types::{EntityPath, TimeInt};
use re_sorbet::ComponentColumnDescriptor;

use crate::common::v1alpha1::{ComponentDescriptor, DataframePart, TaskId};
use crate::manifest_registry::v1alpha1::{
    GetDatasetSchemaResponse, RegisterWithDatasetResponse, ScanPartitionTableResponse,
    VectorDistanceMetric,
};
use crate::v1alpha1::rerun_common_v1alpha1_ext::PartitionId;
use crate::{TypeConversionError, missing_field};

// --- QueryDataset ---

#[derive(Debug, Clone)]
pub struct QueryDatasetRequest {
    pub entry: crate::common::v1alpha1::ext::DatasetHandle,
    pub partition_ids: Vec<crate::common::v1alpha1::ext::PartitionId>,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub select_all_entity_paths: bool,
    pub fuzzy_descriptors: Vec<String>,
    pub exclude_static_data: bool,
    pub exclude_temporal_data: bool,
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

            select_all_entity_paths: value.select_all_entity_paths,

            fuzzy_descriptors: value.fuzzy_descriptors,

            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,

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
                        .and_then(|index| index.timeline.map(|timeline| timeline.name)),
                    at: latest_at
                        .at
                        .map(|at| TimeInt::new_temporal(at))
                        .unwrap_or_else(|| TimeInt::STATIC),
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
            latest_at: value.latest_at.map(Into::into),
            range: value
                .range
                .map(|range| crate::manifest_registry::v1alpha1::QueryRange {
                    index: Some({
                        let timeline: TimelineName = range.index.into();
                        timeline.into()
                    }),
                    index_range: Some(range.index_range.into()),
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
    pub select_all_entity_paths: bool,
    pub fuzzy_descriptors: Vec<String>,
    pub exclude_static_data: bool,
    pub exclude_temporal_data: bool,
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

            select_all_entity_paths: value.select_all_entity_paths,

            fuzzy_descriptors: value.fuzzy_descriptors,

            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,

            query: value.query.map(|q| q.try_into()).transpose()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct QueryLatestAt {
    /// Index name (timeline) to query.
    ///
    /// Use `None` for static only data.
    pub index: Option<String>,

    /// The timestamp to query at.
    ///
    /// Use `TimeInt::STATIC` to query for static only data.
    pub at: TimeInt,
}

impl QueryLatestAt {
    pub fn new_static() -> Self {
        Self {
            index: None,
            at: TimeInt::STATIC,
        }
    }

    pub fn is_static(&self) -> bool {
        self.index.is_none()
    }
}

impl From<QueryLatestAt> for crate::manifest_registry::v1alpha1::QueryLatestAt {
    fn from(value: QueryLatestAt) -> Self {
        crate::manifest_registry::v1alpha1::QueryLatestAt {
            index: value.index.map(|index| {
                let timeline: TimelineName = index.into();
                timeline.into()
            }),
            at: Some(value.at.as_i64()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryRange {
    pub index: String,
    pub index_range: re_log_types::ResolvedTimeRange,
}

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
    pub const PARTITION_LAYER: &str = "rerun_partition_layer";
    pub const PARTITION_TYPE: &str = "rerun_partition_type";
    pub const STORAGE_URL: &str = "rerun_storage_url";
    pub const TASK_ID: &str = "rerun_task_id";

    /// The Arrow schema of the dataframe in [`Self::data`].
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::PARTITION_ID, DataType::Utf8, false),
            Field::new(Self::PARTITION_LAYER, DataType::Utf8, false),
            Field::new(Self::PARTITION_TYPE, DataType::Utf8, false),
            Field::new(Self::STORAGE_URL, DataType::Utf8, false),
            Field::new(Self::TASK_ID, DataType::Utf8, false),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        partition_layers: Vec<String>,
        partition_types: Vec<String>,
        storage_urls: Vec<String>,
        task_ids: Vec<String>,
    ) -> arrow::error::Result<RecordBatch> {
        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(StringArray::from(partition_layers)),
            Arc::new(StringArray::from(partition_types)),
            Arc::new(StringArray::from(storage_urls)),
            Arc::new(StringArray::from(task_ids)),
        ];

        RecordBatch::try_new(schema, columns)
    }
}

//TODO(ab): this should be an actual grpc message, returned by `RegisterWithDataset` instead of a dataframe
#[derive(Debug)]
pub struct RegisterWithDatasetTaskDescriptor {
    pub partition_id: PartitionId,
    pub partition_type: DataSourceKind,
    pub storage_url: url::Url,
    pub task_id: TaskId,
}

// --- ScanPartitionTableResponse --

impl ScanPartitionTableResponse {
    pub const PARTITION_ID: &str = "rerun_partition_id";
    pub const PARTITION_TYPE: &str = "rerun_partition_type";
    pub const STORAGE_URL: &str = "rerun_storage_url";
    pub const REGISTRATION_TIME: &str = "rerun_registration_time";
    pub const PARTITION_MANIFEST_UPDATED_AT: &str = "rerun_partition_manifest_updated_at";
    pub const PARTITION_MANIFEST_URL: &str = "rerun_partition_manifest_url";

    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::PARTITION_ID, DataType::Utf8, false),
            Field::new(Self::PARTITION_TYPE, DataType::Utf8, false),
            Field::new(Self::STORAGE_URL, DataType::Utf8, false),
            Field::new(
                Self::REGISTRATION_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                Self::PARTITION_MANIFEST_UPDATED_AT,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::PARTITION_MANIFEST_URL, DataType::Utf8, true),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        partition_types: Vec<String>,
        storage_urls: Vec<String>,
        registration_times: Vec<i64>,
        partition_manifest_updated_ats: Vec<Option<i64>>,
        partition_manifest_urls: Vec<Option<String>>,
    ) -> arrow::error::Result<RecordBatch> {
        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(StringArray::from(partition_types)),
            Arc::new(StringArray::from(storage_urls)),
            Arc::new(TimestampNanosecondArray::from(registration_times)),
            Arc::new(TimestampNanosecondArray::from(
                partition_manifest_updated_ats,
            )),
            Arc::new(StringArray::from(partition_manifest_urls)),
        ];

        RecordBatch::try_new(schema, columns)
    }

    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self.data.as_ref().ok_or_else(|| {
            missing_field!(
                crate::manifest_registry::v1alpha1::ScanPartitionTableResponse,
                "data"
            )
        })?)
    }
}

// --- DataSource --

// NOTE: Match the values of the Protobuf definition to keep life simple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DataSourceKind {
    Rrd = 1,
}

impl TryFrom<crate::manifest_registry::v1alpha1::DataSourceKind> for DataSourceKind {
    type Error = TypeConversionError;

    fn try_from(
        kind: crate::manifest_registry::v1alpha1::DataSourceKind,
    ) -> Result<Self, Self::Error> {
        match kind {
            crate::manifest_registry::v1alpha1::DataSourceKind::Rrd => Ok(Self::Rrd),

            crate::manifest_registry::v1alpha1::DataSourceKind::Unspecified => {
                return Err(TypeConversionError::InvalidField {
                    package_name: "rerun.manifest_registry.v1alpha1",
                    type_name: "DataSourceKind",
                    field_name: "",
                    reason: "enum value unspecified".to_owned(),
                });
            }
        }
    }
}

impl TryFrom<i32> for DataSourceKind {
    type Error = TypeConversionError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        let kind = crate::manifest_registry::v1alpha1::DataSourceKind::try_from(kind)?;
        kind.try_into()
    }
}

impl From<DataSourceKind> for crate::manifest_registry::v1alpha1::DataSourceKind {
    fn from(value: DataSourceKind) -> Self {
        match value {
            DataSourceKind::Rrd => Self::Rrd,
        }
    }
}

impl DataSourceKind {
    pub fn to_arrow(self) -> ArrayRef {
        match self {
            Self::Rrd => {
                let rec_type = StringArray::from_iter_values(["rrd".to_owned()]);
                Arc::new(rec_type)
            }
        }
    }

    pub fn many_to_arrow(types: Vec<Self>) -> ArrayRef {
        let data = types
            .into_iter()
            .map(|typ| match typ {
                Self::Rrd => "rrd",
            })
            .collect::<Vec<_>>();
        Arc::new(StringArray::from(data))
    }

    pub fn from_arrow(array: &dyn Array) -> Result<Self, TypeConversionError> {
        let resource_type = array.try_downcast_array_ref::<StringArray>()?.value(0);

        match resource_type {
            "rrd" => Ok(Self::Rrd),
            _ => Err(TypeConversionError::ArrowError(
                ArrowError::InvalidArgumentError(format!("unknown resource type {resource_type}")),
            )),
        }
    }

    pub fn many_from_arrow(array: &dyn Array) -> Result<Vec<Self>, TypeConversionError> {
        let string_array = array.try_downcast_array_ref::<StringArray>()?;

        (0..string_array.len())
            .map(|i| {
                let resource_type = string_array.value(i);
                match resource_type {
                    "rrd" => Ok(Self::Rrd),
                    _ => Err(TypeConversionError::ArrowError(
                        ArrowError::InvalidArgumentError(format!(
                            "unknown resource type {resource_type}"
                        )),
                    )),
                }
            })
            .collect()
    }
}

#[test]
fn datasourcekind_roundtrip() {
    let kind = DataSourceKind::Rrd;
    let kind: crate::manifest_registry::v1alpha1::DataSourceKind = kind.into();
    let kind = DataSourceKind::try_from(kind).unwrap();
    assert_eq!(DataSourceKind::Rrd, kind);
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataSource {
    pub storage_url: url::Url,
    pub layer: String,
    pub kind: DataSourceKind,
}

impl DataSource {
    pub const DEFAULT_LAYER: &str = "base";

    pub fn new_rrd(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            layer: Self::DEFAULT_LAYER.to_owned(),
            kind: DataSourceKind::Rrd,
        })
    }

    pub fn new_rrd_layer(
        layer: impl AsRef<str>,
        storage_url: impl AsRef<str>,
    ) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            layer: layer.as_ref().into(),
            kind: DataSourceKind::Rrd,
        })
    }
}

impl From<DataSource> for crate::manifest_registry::v1alpha1::DataSource {
    fn from(value: DataSource) -> Self {
        crate::manifest_registry::v1alpha1::DataSource {
            storage_url: Some(value.storage_url.to_string()),
            layer: Some(value.layer),
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

        let layer = data_source
            .layer
            .unwrap_or_else(|| Self::DEFAULT_LAYER.to_owned());

        let kind = DataSourceKind::try_from(data_source.typ)?;

        Ok(Self {
            storage_url,
            layer,
            kind,
        })
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
                archetype: value.archetype.map(|n| n.full_name().to_owned()),
                component: Some(value.component.to_string()),
                component_type: value.component_type.map(|c| c.full_name().to_owned()),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DoMaintenanceRequest {
    pub entry: crate::common::v1alpha1::ext::DatasetHandle,
    pub build_scalar_indexes: bool,
    pub compact_fragments: bool,
    pub cleanup_before: Option<jiff::Timestamp>,
}

impl TryFrom<crate::manifest_registry::v1alpha1::DoMaintenanceRequest> for DoMaintenanceRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::manifest_registry::v1alpha1::DoMaintenanceRequest,
    ) -> Result<Self, Self::Error> {
        let cleanup_before = value
            .cleanup_before
            .map(|ts| jiff::Timestamp::new(ts.seconds, ts.nanos))
            .transpose()?;

        Ok(Self {
            entry: value
                .entry
                .ok_or_else(|| {
                    TypeConversionError::missing_field::<
                        crate::manifest_registry::v1alpha1::DoMaintenanceRequest,
                    >("entry")
                })?
                .try_into()?,
            build_scalar_indexes: value.build_scalar_indexes,
            compact_fragments: value.compact_fragments,
            cleanup_before,
        })
    }
}

impl From<DoMaintenanceRequest> for crate::manifest_registry::v1alpha1::DoMaintenanceRequest {
    fn from(value: DoMaintenanceRequest) -> Self {
        Self {
            entry: Some(value.entry.into()),
            build_scalar_indexes: value.build_scalar_indexes,
            compact_fragments: value.compact_fragments,
            cleanup_before: value.cleanup_before.map(|ts| prost_types::Timestamp {
                seconds: ts.as_second(),
                nanos: ts.subsec_nanosecond(),
            }),
        }
    }
}
