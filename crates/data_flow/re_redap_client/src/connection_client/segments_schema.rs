use std::borrow::Borrow;
use std::collections::BTreeSet;

use arrow::datatypes::Schema as ArrowSchema;
use itertools::Itertools as _;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{LayerRegistrationStatus, ScanDatasetManifestDataframe};
use re_protos::common::v1alpha1::ext::{SchemaId, SegmentId};
use re_protos::external::prost::bytes::Bytes;
use re_types_core::LayerName;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiErrorKind, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    /// The union of the schemas of the registered layers of `segment_ids`, or `None` if the
    /// dataset holds none of them.
    ///
    /// The order of the fields is unspecified.
    ///
    /// Against servers without `/GetSchemas`, falls back to one `/GetRrdManifest` per segment.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segments_schema(
        &mut self,
        dataset_id: EntryId,
        segment_ids: &BTreeSet<SegmentId>,
    ) -> ApiResult<Option<ArrowSchema>> {
        if segment_ids.is_empty() {
            return Ok(None);
        }

        self.get_registered_layers_schema(
            dataset_id,
            LayerRows {
                segment_ids: Some(segment_ids),
                layer_name: None,
            },
        )
        .await
    }

    /// The union of the schemas of `layer` in every segment that has it registered, or `None` if
    /// none has.
    ///
    /// The order of the fields is unspecified.
    ///
    /// Servers without `/GetSchemas` fail with [`ApiErrorKind::Unimplemented`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_layer_schema(
        &mut self,
        dataset_id: EntryId,
        layer: &LayerName,
    ) -> ApiResult<Option<ArrowSchema>> {
        self.get_registered_layers_schema(
            dataset_id,
            LayerRows {
                segment_ids: None,
                layer_name: Some(layer),
            },
        )
        .await
    }

    async fn get_registered_layers_schema(
        &mut self,
        dataset_id: EntryId,
        rows: LayerRows<'_>,
    ) -> ApiResult<Option<ArrowSchema>> {
        // A layer unregistered between the manifest scan and the lookup can take its schema with
        // it, so a missing schema gets one retry against a fresh manifest.
        let mut retried = false;
        loop {
            let manifest = self.layer_schema_ids(dataset_id, rows).await?;
            if manifest.segment_ids.is_empty() {
                return Ok(None);
            }

            let schemas = match self.get_schemas(dataset_id, &manifest.schema_ids).await {
                Ok(schemas) => schemas,
                Err(err) if err.kind == ApiErrorKind::Unimplemented => {
                    // An RRD manifest merges all of a segment's layers.
                    if let Some(layer_name) = rows.layer_name {
                        return Err(ApiError::unimplemented(
                            &self.origin,
                            format!(
                                "the schema of layer '{layer_name}' requires a server running \
                                 Rerun 0.41 or Rerun Hub 0.20 or later"
                            ),
                        ));
                    }
                    return self
                        .get_segments_schema_from_rrd_manifests(dataset_id, &manifest.segment_ids)
                        .await
                        .map(Some);
                }
                Err(err) => return Err(err),
            };

            let missing: Vec<_> = manifest
                .schema_ids
                .iter()
                .filter(|schema_id| !schemas.contains_key(schema_id))
                .collect();
            if missing.is_empty() {
                return self.merge_schema_fields(schemas.into_values()).map(Some);
            }

            if retried {
                return Err(ApiError::failed_precondition(
                    &self.origin,
                    format!(
                        "/GetSchemas did not return schemas listed in the manifest of dataset \
                         {dataset_id}, which may be changing concurrently: {}",
                        missing.iter().join(", ")
                    ),
                ));
            }
            retried = true;
        }
    }

    /// The schema IDs of the registered layers selected by `rows`, from the dataset manifest.
    async fn layer_schema_ids(
        &mut self,
        dataset_id: EntryId,
        rows: LayerRows<'_>,
    ) -> ApiResult<LayerSchemaIds> {
        let columns = [
            ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME_NAME,
            ScanDatasetManifestDataframe::COLUMN_RERUN_SCHEMA_SHA256_NAME,
            ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_STATUS_NAME,
        ];
        let layer_names = rows
            .layer_name
            .map(|layer_name| BTreeSet::from([layer_name.clone()]));
        let manifest = self
            .scan_dataset_manifest(dataset_id, columns, rows.segment_ids, layer_names.as_ref())
            .await?;

        let decode_failed = |err| {
            ApiError::deserialization_quiver_from(&self.origin, None, err, "/ScanDatasetManifest")
        };
        let mut layer_schema_ids = LayerSchemaIds {
            schema_ids: BTreeSet::new(),
            segment_ids: BTreeSet::new(),
        };

        for batch in &manifest.batches {
            let ids = ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(batch)
                .map_err(decode_failed)?;
            let layer_names = ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
                .extract(batch)
                .map_err(decode_failed)?;
            let schema_sha256s = ScanDatasetManifestDataframe::COLUMN_RERUN_SCHEMA_SHA256
                .extract(batch)
                .map_err(decode_failed)?;
            let statuses = ScanDatasetManifestDataframe::COLUMN_RERUN_REGISTRATION_STATUS
                .extract(batch)
                .map_err(decode_failed)?;

            let batch_rows = itertools::izip!(
                ids.iter_owned(),
                layer_names.iter_owned(),
                schema_sha256s.iter_owned(),
                statuses.iter_owned()
            );
            for (segment_id, layer_name, schema_sha256, status) in batch_rows {
                // Servers may ignore the filter hints, and errored registrations stay listed.
                if !rows.contains(&segment_id, &layer_name)
                    || status != LayerRegistrationStatus::Done.as_str()
                {
                    continue;
                }
                let Some(schema_sha256) = schema_sha256 else {
                    return Err(ApiError::deserialization(
                        &self.origin,
                        None,
                        format!(
                            "/ScanDatasetManifest listed a registered layer of segment \
                             '{segment_id}' without a schema hash"
                        ),
                    ));
                };
                layer_schema_ids
                    .schema_ids
                    .insert(SchemaId::from_sha256(schema_sha256));
                layer_schema_ids.segment_ids.insert(segment_id);
            }
        }

        Ok(layer_schema_ids)
    }

    async fn get_segments_schema_from_rrd_manifests(
        &mut self,
        dataset_id: EntryId,
        segment_ids: &BTreeSet<SegmentId>,
    ) -> ApiResult<ArrowSchema> {
        let mut schemas = Vec::with_capacity(segment_ids.len());
        for segment_id in segment_ids {
            let manifest = self
                .get_rrd_manifest(dataset_id, segment_id.clone())
                .await?;
            schemas.push(manifest.sorbet_schema);
        }
        self.merge_schema_fields(schemas)
    }

    fn merge_schema_fields(
        &self,
        schemas: impl IntoIterator<Item = impl Borrow<ArrowSchema>>,
    ) -> ApiResult<ArrowSchema> {
        re_arrow_util::merge_schema_fields(schemas).map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                None,
                err,
                "failed merging layer schemas",
            )
        })
    }
}

/// Which rows of the dataset manifest to take the schemas of; `None` keeps every value.
#[derive(Clone, Copy)]
struct LayerRows<'a> {
    segment_ids: Option<&'a BTreeSet<SegmentId>>,
    layer_name: Option<&'a LayerName>,
}

impl LayerRows<'_> {
    fn contains(&self, segment_id: &SegmentId, layer_name: &LayerName) -> bool {
        self.segment_ids
            .is_none_or(|segment_ids| segment_ids.contains(segment_id))
            && self.layer_name.is_none_or(|layer| layer == layer_name)
    }
}

/// What the dataset manifest says about some registered layers.
struct LayerSchemaIds {
    schema_ids: BTreeSet<SchemaId>,

    /// The segments that have at least one of the selected layers registered.
    segment_ids: BTreeSet<SegmentId>,
}
