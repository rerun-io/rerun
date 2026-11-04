use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::datatypes::Schema as ArrowSchema;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::GetSchemasRequest;
use re_protos::cloud::v1alpha1::ext as cloud_ext;
use re_protos::common::v1alpha1::ext::SchemaId;
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiErrorKind, ApiResponseStream, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Look up the schemas of a dataset's layers by ID, as found in its manifest.
    ///
    /// IDs the dataset does not hold are absent from the result, unless an earlier lookup on this
    /// connection, for any dataset, already returned them.
    ///
    /// Schemas are cached for the lifetime of the connection to this server, since an ID is the
    /// hash of its schema.
    /// Only IDs not looked up before reach the server, split across as many `/GetSchemas` calls as
    /// [`cloud_ext::GetSchemasRequest::MAX_SCHEMA_IDS`] requires.
    ///
    /// Servers without `/GetSchemas` fail with [`ApiErrorKind::Unimplemented`].
    #[tracing::instrument(level = "info", skip_all, fields(num_requested = schema_ids.len()))]
    pub async fn get_schemas(
        &mut self,
        entry_id: EntryId,
        schema_ids: &BTreeSet<SchemaId>,
    ) -> ApiResult<BTreeMap<SchemaId, Arc<ArrowSchema>>> {
        if self.schema_cache.get_schemas_unimplemented() {
            return Err(ApiError::unimplemented(
                &self.origin,
                "the server does not implement /GetSchemas",
            ));
        }

        let (mut schemas, missing) = self.schema_cache.split_cached(schema_ids);
        let missing: Vec<SchemaId> = missing.into_iter().collect();

        for chunk in missing.chunks(cloud_ext::GetSchemasRequest::MAX_SCHEMA_IDS) {
            let fetched = match self
                .get_schemas_single_request(entry_id, chunk.iter().copied().collect())
                .await
            {
                Ok(fetched) => fetched,
                Err(err) => {
                    if err.kind == ApiErrorKind::Unimplemented {
                        self.schema_cache.set_get_schemas_unimplemented();
                    }
                    return Err(err);
                }
            };
            self.schema_cache.insert(&fetched);
            schemas.extend(fetched);
        }

        Ok(schemas)
    }

    /// One `/GetSchemas` call, for at most [`cloud_ext::GetSchemasRequest::MAX_SCHEMA_IDS`] IDs.
    async fn get_schemas_single_request(
        &mut self,
        entry_id: EntryId,
        schema_ids: BTreeSet<SchemaId>,
    ) -> ApiResult<BTreeMap<SchemaId, Arc<ArrowSchema>>> {
        let request = GetSchemasRequest::from(cloud_ext::GetSchemasRequest {
            schema_ids: schema_ids.clone(),
        });
        let client = self.inner().clone();
        let origin = &self.origin;

        // Retry only the open: admission control rejects the call before the stream exists.
        let response = crate::with_retry_resource_exhausted("/GetSchemas", || {
            let client = client.clone();
            let request = request.clone();
            async move {
                crate::rpc_retry::retry(|| {
                    let mut client = client.clone();
                    let request = request.clone();
                    async move {
                        crate::rpc_retry::open_stream(
                            client
                                .get_schemas(tonic::Request::new(request).with_entry_id(entry_id))
                                .await?,
                        )
                        .await
                    }
                })
                .await
                .map_err(|err| ApiError::tonic(origin, err, "/GetSchemas"))
            }
        })
        .await?;

        let mut stream =
            ApiResponseStream::from_tonic_response(self.origin.clone(), response, "/GetSchemas")
                .inspect_first(|response| {
                    crate::dataset_revisions().observe_meta(response.meta.as_ref());
                });
        let trace_id = stream.trace_id();

        let mut schemas = BTreeMap::new();

        while let Some(response) = stream.next().await {
            let response = response?;

            let cloud_ext::GetSchemasResponse { schema_id, schema } =
                response.try_into().map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed decoding item from /GetSchemas stream",
                    )
                })?;
            if !schema_ids.contains(&schema_id) {
                return Err(ApiError::deserialization(
                    &self.origin,
                    trace_id,
                    format!("/GetSchemas returned schema {schema_id}, which was not requested"),
                ));
            }
            schemas.insert(schema_id, Arc::new(schema));
        }

        Ok(schemas)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;

    use super::*;

    /// The client points at an unrouteable address, so any `/GetSchemas` call would fail.
    fn client_without_server() -> RedapClient<tonic::transport::Channel> {
        let channel = tonic::transport::Channel::from_static("http://127.0.0.1:1").connect_lazy();
        RedapClient::new(
            re_uri::Origin::test(),
            RerunCloudServiceClient::new(channel),
            None,
        )
    }

    #[tokio::test]
    async fn get_schemas_serves_cached_ids_without_the_server() {
        let client = client_without_server();
        let schema_id = SchemaId::from_sha256([1; 32]);
        let schema = Arc::new(ArrowSchema::new_with_metadata(
            vec![arrow::datatypes::Field::new(
                "a",
                arrow::datatypes::DataType::Int64,
                true,
            )],
            Default::default(),
        ));
        client
            .schema_cache
            .insert(&BTreeMap::from([(schema_id, Arc::clone(&schema))]));

        // A clone shares the cache, as the clients handed out for one connection do.
        let schemas = client
            .clone()
            .get_schemas(EntryId::new(), &BTreeSet::from([schema_id]))
            .await
            .unwrap();

        assert_eq!(schemas, BTreeMap::from([(schema_id, schema)]));
    }

    #[tokio::test]
    async fn get_schemas_fails_without_the_server_once_unimplemented() {
        let mut client = client_without_server();
        client.schema_cache.set_get_schemas_unimplemented();

        let err = client
            .get_schemas(
                EntryId::new(),
                &BTreeSet::from([SchemaId::from_sha256([1; 32])]),
            )
            .await
            .unwrap_err();

        assert_eq!(err.kind, ApiErrorKind::Unimplemented);
    }
}
