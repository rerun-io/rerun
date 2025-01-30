use arrow::array::RecordBatch as ArrowRecordBatch;
use arrow::datatypes::Schema as ArrowSchema;

use re_log_types::EntityPath;

#[derive(thiserror::Error, Debug)]
pub enum ChunkBatchError {
    #[error("Missing record batch metadata key `{0}`")]
    MissingRecordBatchMetadataKey(&'static str),
}

/// The arrow [`ArrowRecordBatch`] representation of a Rerun chunk.
///
/// This is a wrapper around a [`ArrowRecordBatch`].
///
/// Each [`ChunkBatch`] contains logging data for a single [`EntityPath`].
/// It walwyas have a [`RowId`] column.
#[derive(Debug, Clone)]
pub struct ChunkBatch {
    entity_path: EntityPath,
    batch: ArrowRecordBatch,
}

impl std::fmt::Display for ChunkBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_format_arrow::format_record_batch_with_width(self, f.width()).fmt(f)
    }
}

impl AsRef<ArrowRecordBatch> for ChunkBatch {
    #[inline]
    fn as_ref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl std::ops::Deref for ChunkBatch {
    type Target = ArrowRecordBatch;

    #[inline]
    fn deref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl From<ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: ChunkBatch) -> Self {
        chunk.batch
    }
}

impl TryFrom<ArrowRecordBatch> for ChunkBatch {
    type Error = ChunkBatchError;

    fn try_from(batch: ArrowRecordBatch) -> Result<Self, Self::Error> {
        let mut schema = ArrowSchema::clone(&*batch.schema());
        let metadata = &mut schema.metadata;

        {
            // Verify version
            if let Some(batch_version) = metadata.get(Self::CHUNK_METADATA_KEY_VERSION) {
                if batch_version != Self::CHUNK_METADATA_VERSION {
                    re_log::warn_once!(
                        "ChunkBatch version mismatch. Expected {:?}, got {batch_version:?}",
                        Self::CHUNK_METADATA_VERSION
                    );
                }
            }
            metadata.insert(
                Self::CHUNK_METADATA_KEY_VERSION.to_owned(),
                Self::CHUNK_METADATA_VERSION.to_owned(),
            );
        }

        let entity_path =
            if let Some(entity_path) = metadata.get(Self::CHUNK_METADATA_KEY_ENTITY_PATH) {
                EntityPath::parse_forgiving(entity_path)
            } else {
                return Err(ChunkBatchError::MissingRecordBatchMetadataKey(
                    Self::CHUNK_METADATA_KEY_ENTITY_PATH,
                ));
            };

        Ok(Self { entity_path, batch })
    }
}

/// ## Metadata keys for the record batch metadata
impl ChunkBatch {
    /// The key used to identify the version of the Rerun schema.
    const CHUNK_METADATA_KEY_VERSION: &'static str = "rerun.version";

    /// The version of the Rerun schema.
    const CHUNK_METADATA_VERSION: &'static str = "1";

    /// The key used to identify a Rerun [`EntityPath`] in the record batch metadata.
    const CHUNK_METADATA_KEY_ENTITY_PATH: &'static str = "rerun.entity_path";
}

impl ChunkBatch {
    /// Returns the [`EntityPath`] of the batch.
    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }
}
