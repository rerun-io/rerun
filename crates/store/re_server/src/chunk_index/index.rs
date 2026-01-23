use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, DictionaryArray, FixedSizeBinaryArray, Int64Array, Int64BufferBuilder,
    RecordBatch, RecordBatchIterator, StringArray, UInt32Array, UInt32BufferBuilder,
};
use arrow::buffer::ScalarBuffer;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::error::ArrowError;
use lance::deps::arrow_array::UInt8Array;
use re_chunk_store::Chunk;
use re_log_types::{EntityPath, TimelineName};
use re_protos::cloud::v1alpha1::ext::{IndexConfig, IndexProperties};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_types_core::ComponentIdentifier;

use crate::chunk_index::{
    ArcCell, FIELD_CHUNK_ID, FIELD_INSTANCE, FIELD_INSTANCE_ID, FIELD_RERUN_SEGMENT_ID,
    FIELD_RERUN_SEGMENT_LAYER, FIELD_TIMEPOINT,
};
use crate::store::{Dataset, Error as StoreError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexType {
    Inverted,
    VectorIvfPq,
    BTree,
}

/// Arrow types for indexed data coming from a chunk.
pub struct IndexDataTypes {
    pub instances: DataType,
    pub timepoints: DataType,
}

impl From<&IndexProperties> for IndexType {
    fn from(properties: &IndexProperties) -> Self {
        match properties {
            IndexProperties::Inverted { .. } => Self::Inverted,
            IndexProperties::VectorIvfPq { .. } => Self::VectorIvfPq,
            IndexProperties::Btree => Self::BTree,
        }
    }
}

impl super::Index {
    /// Store chunks in the index.
    pub async fn store_chunks(
        &self,
        chunks: Vec<(SegmentId, String, Arc<Chunk>)>,
        checkout_latest: bool,
    ) -> Result<(), StoreError> {
        let index_type: IndexType = (&self.config.properties).into();
        let timeline = self.config.time_index;
        let component = self.config.column.descriptor.component;

        let batches = chunks
            .into_iter()
            .filter_map(move |(segment_id, layer, chunk)| {
                Self::prepare_record_batch(
                    index_type,
                    &segment_id,
                    layer,
                    timeline,
                    component,
                    &chunk,
                )
                .transpose()
            });

        let mut lance: lance::Dataset = self.lance_dataset.cloned();
        let mut iter = batches.peekable();

        // Expect the first batch to be successfully prepared to get its schema.
        if let Some(Ok(first)) = iter.peek() {
            let schema = first.schema();
            lance
                .append(
                    RecordBatchIterator::new(iter, schema),
                    Some(Default::default()),
                )
                .await?;

            // TODO(swallez) we should call optimize_indices and compact_files sometimes.
            // We can either do it every X insertions or use a debouncer to enforce a max frequency.

            if checkout_latest {
                lance.checkout_latest().await?;
                self.lance_dataset.replace(lance);
            }
        } else {
            Err(StoreError::IndexingError(
                "Cannot determine indexed data schema".to_owned(),
            ))?;
        }

        Ok(())
    }

    /// Prepare a record batch for a chunk, given a timeline and component. Other parameters are used
    /// to add source information to the indexed instance values.
    ///
    /// Returns `None` if the chunk doesn't contain the `timeline` or the `component`.
    pub fn prepare_record_batch(
        index_type: IndexType,
        segment_id: &SegmentId,
        layer: String,
        timeline: TimelineName,
        component: ComponentIdentifier,
        chunk: &Arc<Chunk>,
    ) -> Result<Option<RecordBatch>, ArrowError> {
        let Some(timeline) = chunk.timelines().get(&timeline) else {
            // No such timeline
            return Ok(None);
        };

        let Some(component) = chunk.components().get(component) else {
            // No such component
            return Ok(None);
        };

        // Nominal cases: each row is a list of instance values.
        // The other case is Vector indexing with rows being a single vector containing numbers
        let row_is_array_of_instances = match index_type {
            IndexType::Inverted | IndexType::BTree => true,
            // see also `find_datatypes`
            IndexType::VectorIvfPq if !component.list_array.value_type().is_numeric() => true,
            IndexType::VectorIvfPq => false,
        };

        // To pre-size buffers and avoid reallocations.
        let total_instances = if row_is_array_of_instances {
            component
                .list_array
                .iter()
                .map(|x| x.map(|x| x.len()).unwrap_or(0))
                .sum()
        } else {
            component.list_array.len() - component.list_array.null_count()
        };

        // Dictionary encoding of values repeated for each row. The keys are all zeroes, pointing
        // to the first element of the dictionary values array.
        let dict_keys = UInt8Array::from_iter_values(std::iter::repeat_n(0, total_instances));

        let segment_id_array = {
            let segment_id_values = StringArray::from_iter_values([segment_id.id.as_str()]);
            DictionaryArray::new(dict_keys.clone(), Arc::new(segment_id_values))
        };

        let layer_array = {
            let layer_values = StringArray::from_iter_values([layer]);
            DictionaryArray::new(dict_keys.clone(), Arc::new(layer_values))
        };

        let chunk_id_array = FixedSizeBinaryArray::try_from_iter(std::iter::repeat_n(
            chunk.id().as_bytes(),
            total_instances,
        ))?;

        let instance_id_array: UInt32Array;
        let timepoint_array: ArrayRef;
        let instance_array: ArrayRef;

        if row_is_array_of_instances {
            let mut timepoints = Int64BufferBuilder::new(total_instances);
            let mut instance_ids = UInt32BufferBuilder::new(total_instances);

            // Collect instance arrays, they will be concatenated later.
            let mut instances = Vec::new();

            for (row_num, instance) in component.list_array.iter().enumerate() {
                let Some(instance) = instance else {
                    continue;
                };

                // Repeat time as many times as there are instances in the row.
                timepoints.append_n(instance.len(), timeline.times_raw()[row_num]);

                for i in 0..instance.len() as u32 {
                    instance_ids.append(i);
                }
                instances.push(instance);
            }

            // Note: no support for 64-bit seconds and millis, but time-based timelines use nanos.
            timepoint_array = arrow::compute::cast(
                &Int64Array::new(ScalarBuffer::from(timepoints), None),
                &timeline.timeline().datatype(),
            )?;

            let instance_arrays: Vec<&dyn Array> = instances.iter().map(|x| x.as_ref()).collect();
            instance_array = re_arrow_util::concat_arrays(instance_arrays.as_slice())?;

            instance_id_array = UInt32Array::new(ScalarBuffer::from(instance_ids), None);
        } else {
            // All rows are a single vector. Just filter out nulls, if any.
            if component.list_array.null_count() == 0 {
                instance_array = Arc::new(component.list_array.clone());
                timepoint_array = Arc::new(timeline.times_array().clone());
            } else {
                let non_nulls = arrow::compute::is_not_null(&component.list_array)?;

                let list_array: ArrayRef = Arc::new(component.list_array.clone());
                instance_array = re_arrow_util::filter_array(&list_array, &non_nulls);
                timepoint_array = re_arrow_util::filter_array(&timeline.times_array(), &non_nulls);
            }

            let mut instance_ids = UInt32BufferBuilder::new(total_instances);
            // One instance per row => instance ids are all zero.
            instance_ids.append_n(total_instances, 0);
            instance_id_array = UInt32Array::new(ScalarBuffer::from(instance_ids), None);
        }

        // Keep in sync (including types) with `create_lance_dataset`
        let batch = RecordBatch::try_from_iter([
            (
                FIELD_RERUN_SEGMENT_ID,
                Arc::new(segment_id_array) as ArrayRef,
            ),
            (FIELD_RERUN_SEGMENT_LAYER, Arc::new(layer_array)),
            (FIELD_CHUNK_ID, Arc::new(chunk_id_array)),
            (FIELD_TIMEPOINT, Arc::new(timepoint_array)),
            (FIELD_INSTANCE_ID, Arc::new(instance_id_array)),
            (FIELD_INSTANCE, Arc::new(instance_array)),
        ])?;

        Ok(Some(batch))
    }
}

/// Create an index
pub async fn create_index(
    dataset: &Dataset,
    config: &IndexConfig,
    path: PathBuf,
) -> Result<super::Index, StoreError> {
    let index_type: IndexType = (&config.properties).into();
    let types: IndexDataTypes = find_datatypes(
        dataset,
        index_type,
        &config.column.entity_path,
        &config.column.descriptor.component,
        &config.time_index,
    )
    .ok_or_else(|| {
        StoreError::EntryNameNotFound(format!(
            "{}#{}",
            config.column.entity_path, config.column.descriptor.component
        ))
    })?;

    let mut lance_table = create_lance_dataset(&path, types).await?;

    create_lance_index(&mut lance_table, &config.properties).await?;

    Ok(super::Index {
        lance_dataset: ArcCell::new(lance_table),
        config: config.clone(),
    })
}

async fn create_lance_dataset(
    path: &Path,
    types: IndexDataTypes,
) -> Result<lance::Dataset, StoreError> {
    let non_nullable = false;

    let schema = Arc::new(
        // Keep in sync with `prepare_record_batch`
        #[expect(clippy::disallowed_methods)]
        Schema::new(vec![
            // Chunk identification values are the same for all rows: use a dictionary
            Field::new_dictionary(
                FIELD_RERUN_SEGMENT_ID,
                DataType::UInt8,
                DataType::Utf8,
                non_nullable,
            )
            .with_dict_is_ordered(true),
            Field::new_dictionary(
                FIELD_RERUN_SEGMENT_LAYER,
                DataType::UInt8,
                DataType::Utf8,
                non_nullable,
            )
            .with_dict_is_ordered(true),
            // Will be repeated, but Lance doesn't support dictionaries for FixedSizeBinary because
            // of stringly-typed checks in lance_core::data_types (look for "Unsupported dictionary type")
            Field::new(FIELD_CHUNK_ID, DataType::FixedSizeBinary(16), non_nullable)
                .with_dict_is_ordered(true),
            Field::new(FIELD_TIMEPOINT, types.timepoints, non_nullable),
            // Position of the instance value that matched the query. Arrow lists use 32-bit offsets.
            Field::new(FIELD_INSTANCE_ID, DataType::UInt32, non_nullable),
            Field::new(FIELD_INSTANCE, types.instances, true),
        ]),
    );

    let batch = RecordBatch::new_empty(schema.clone());
    let batches = RecordBatchIterator::new(vec![batch].into_iter().map(Ok), schema.clone());

    let dataset = lance::Dataset::write(batches, path.to_string_lossy().as_ref(), None).await?;

    Ok(dataset)
}

async fn create_lance_index(
    lance_table: &mut lance::Dataset,
    properties: &IndexProperties,
) -> Result<(), StoreError> {
    use lance::index::vector::VectorIndexParams;
    use lance_index::scalar::{InvertedIndexParams, ScalarIndexParams};
    use lance_index::{DatasetIndexExt as _, IndexParams, IndexType};
    use lance_linalg::distance::MetricType;
    use re_protos::cloud::v1alpha1::VectorDistanceMetric;

    // Convert index properties
    let (index_type, index_params): (IndexType, &dyn IndexParams) = match properties {
        IndexProperties::Inverted {
            store_position,
            base_tokenizer,
        } => (
            IndexType::Inverted,
            &InvertedIndexParams::default()
                .with_position(*store_position)
                .base_tokenizer(base_tokenizer.clone()),
        ),

        IndexProperties::VectorIvfPq {
            target_partition_num_rows,
            num_sub_vectors,
            metric,
        } => {
            let ivf_params = lance_index::vector::ivf::IvfBuildParams {
                target_partition_size: target_partition_num_rows.map(|v| v as usize),
                ..Default::default()
            };

            let pq_params = lance_index::vector::pq::PQBuildParams {
                num_sub_vectors: *num_sub_vectors as usize,
                ..Default::default()
            };

            let lance_metric = match metric {
                VectorDistanceMetric::Unspecified => {
                    return Err(StoreError::IndexingError(
                        "Unspecified distance metric".to_owned(),
                    ));
                }
                VectorDistanceMetric::L2 => MetricType::L2,
                VectorDistanceMetric::Cosine => MetricType::Cosine,
                VectorDistanceMetric::Dot => MetricType::Dot,
                VectorDistanceMetric::Hamming => MetricType::Hamming,
            };

            (
                IndexType::Vector,
                &VectorIndexParams::with_ivf_pq_params(lance_metric, ivf_params, pq_params),
            )
        }

        IndexProperties::Btree => (IndexType::BTree, &ScalarIndexParams::default()),
    };

    match lance_table
        .create_index(&["instance"], index_type, None, index_params, false)
        .await
    {
        Ok(_) => Ok(()),

        // Some failures are expected and ok
        Err(lance::Error::Index { message, .. }) if message.contains("already exists") => Ok(()),

        Err(lance::Error::Index { ref message, .. })
            if message.contains("Not enough rows to train PQ")
                || message.contains("KMeans: can not train") =>
        {
            tracing::warn!("not enough rows to train index yet");
            Ok(())
        }

        Err(lance::Error::NotSupported { source, .. })
            if source
                .to_string()
                .contains("empty vector indices with train=False") =>
        {
            tracing::warn!("not enough rows to train index yet");
            Ok(())
        }
        Err(err) => Err(err),
    }?;

    Ok(())
}

/// Find the datatype of a column by looking up the first chunk containing it.
fn find_datatypes(
    dataset: &Dataset,
    index_type: IndexType,
    entity_path: &EntityPath,
    component: &ComponentIdentifier,
    timeline_name: &TimelineName,
) -> Option<IndexDataTypes> {
    for segment in dataset.segments().values() {
        for layer in segment.layers().values() {
            let chunk_store = layer.store_handle().read();
            for chunk in chunk_store.iter_physical_chunks() {
                if chunk.entity_path() == entity_path
                    && let Some(component) = chunk.components().0.get(component)
                    && let Some(timeline) = chunk.timelines().get(timeline_name)
                {
                    let instance_type = if index_type == IndexType::VectorIvfPq
                        && component.list_array.value_type().is_numeric()
                    {
                        // Row is a single vector, not a list of instances.
                        // See also `prepare_record_batch`.
                        component.list_array.data_type().clone()
                    } else {
                        component.list_array.value_type()
                    };
                    return Some(IndexDataTypes {
                        instances: instance_type,
                        timepoints: timeline.timeline().datatype(),
                    });
                }
            }
        }
    }
    None
}
