use std::collections::HashMap;

use re_log_types::{ApplicationId, RecordingId, StoreId, StoreInfo, StoreKind};
use re_protos::common::v1alpha1::ext::StoreIdMissingApplicationIdError;

/// Application id cache helper for migrating old data.
///
/// For a long time, `StoreId` didn't contain an application id, which was only provided by
/// `StoreInfo`. As a result, messages such as `ArrowMsg` didn't contain an application id. Only
/// `SetStoreInfo` did. To decode these legacy messages, we must remember the application id from
/// `SetStoreInfo` and inject it into later messages.
///
/// This cache is a helper to do so.
//TODO(#10730): this should be entirely suppressed when removing 0.24 back compat
#[derive(Default)]
pub struct ApplicationIdCache(HashMap<(RecordingId, StoreKind), ApplicationId>);

impl ApplicationIdCache {
    /// Populate the cache based on a [`re_log_types::SetStoreInfo`] payload.
    ///
    /// These messages can be gracefully migrated because they used to contain the application id
    /// next to the (legacy, application-id-less) `StoreId`. We can thus populate the cache from
    /// them.
    pub fn insert(&mut self, store_info: &StoreInfo) {
        self.0.insert(
            (store_info.recording_id().clone(), store_info.store_id.kind),
            store_info.application_id().clone(),
        );
    }

    /// Looks up a store id based on a recording id and store kind.
    pub fn recover_store_id(
        &self,
        store_id_err: StoreIdMissingApplicationIdError,
    ) -> Option<StoreId> {
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
