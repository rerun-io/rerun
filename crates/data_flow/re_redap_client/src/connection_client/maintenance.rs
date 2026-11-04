use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext as cloud_ext;
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn do_maintenance(
        &mut self,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> ApiResult {
        self.inner()
            .do_maintenance(
                tonic::Request::new(
                    cloud_ext::DoMaintenanceRequest {
                        optimize_indexes,
                        retrain_indexes,
                        compact_fragments,
                        cleanup_before,
                        gc_object_store: false,
                        unsafe_allow_recent_cleanup,
                    }
                    .into(),
                )
                .with_entry_id(dataset_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoMaintenance"))?;

        Ok(())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn do_global_maintenance(&mut self) -> ApiResult {
        self.inner()
            .do_global_maintenance(tonic::Request::new(
                re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest {},
            ))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoGlobalMaintenance"))?;

        Ok(())
    }
}
