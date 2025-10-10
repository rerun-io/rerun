use std::collections::HashMap;

use re_log_types::{ApplicationId, RecordingId, StoreId, StoreInfo, StoreKind};
use re_protos::common::v1alpha1::ext::StoreIdMissingApplicationIdError;

/// Helper trait for injecting application ids to legacy `StoreId` protobuf messages which miss it.
///
/// Before 0.25, the `StoreId` protobuf didn't contain an application id, which was only provided by
/// `StoreInfo`. As a result, messages such as `ArrowMsg` didn't contain an application id. Only
/// `SetStoreInfo` did. This helper trait expose an interface to cache the application id from
/// `SetStoreInfo` and inject it into later messages.
///
/// Note: this is a trait to allow disabling this mechanism and injecting dummy application ids
/// instead, see [`DummyApplicationIdInjector`], which is needed on redap side.
//TODO(#10730): this should be entirely suppressed when removing 0.24 back compat
pub trait ApplicationIdInjector {
    /// Populate the cache based on a [`re_log_types::SetStoreInfo`] payload.
    fn store_info_received(&mut self, store_info: &StoreInfo);

    /// Try to recover a `StoreId` from a `StoreIdMissingApplicationIdError`.
    fn recover_store_id(&self, store_id_err: StoreIdMissingApplicationIdError) -> Option<StoreId>;
}

/// Implements [`ApplicationIdInjector`] by caching the application ids from `StoreInfo`.
#[derive(Default)]
pub struct CachingApplicationIdInjector(HashMap<(RecordingId, StoreKind), ApplicationId>);

impl ApplicationIdInjector for CachingApplicationIdInjector {
    fn store_info_received(&mut self, store_info: &StoreInfo) {
        self.0.insert(
            (
                store_info.recording_id().clone(),
                store_info.store_id.kind(),
            ),
            store_info.application_id().clone(),
        );
    }

    fn recover_store_id(&self, store_id_err: StoreIdMissingApplicationIdError) -> Option<StoreId> {
        let StoreIdMissingApplicationIdError {
            store_kind,
            recording_id,
        } = store_id_err;

        self.0
            .get(&(recording_id.clone(), store_kind))
            .cloned()
            .map(|app_id| StoreId::new(store_kind, app_id, recording_id))
    }
}

/// Implements [`ApplicationIdInjector`] by returning a constant, dummy application id.
///
/// Do not use this unless you are sure that the application id is not needed.
pub struct DummyApplicationIdInjector {
    application_id: ApplicationId,
}

impl DummyApplicationIdInjector {
    pub fn new(application_id: impl Into<ApplicationId>) -> Self {
        Self {
            application_id: application_id.into(),
        }
    }
}

impl ApplicationIdInjector for DummyApplicationIdInjector {
    fn store_info_received(&mut self, _store_info: &StoreInfo) {
        // No-op, as this is a dummy injector.
    }

    fn recover_store_id(&self, store_id_err: StoreIdMissingApplicationIdError) -> Option<StoreId> {
        Some(StoreId::new(
            store_id_err.store_kind,
            self.application_id.clone(),
            store_id_err.recording_id,
        ))
    }
}
