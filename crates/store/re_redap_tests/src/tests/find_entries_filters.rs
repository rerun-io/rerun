//! Conformance tests for `FindEntries` kind filtering (`EntryFilter.entry_kinds`).
//!
//! Assertions are deliberately robust to the two servers under test (`re_server` and the
//! Rerun Hub frontend) returning different overall entry counts (e.g. the virtual
//! `__entries` system table, which has kind `Table`): we check "all returned kinds are within
//! the requested set" and "the entries we created are present", rather than exact totals.

use re_log_types::EntryName;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{EntryDetails, EntryFilter, EntryKind, FindEntriesRequest};

use super::common::{RerunCloudServiceExt as _, create_table_entry_with_name};

/// Fire a `FindEntries` request and return the raw entries, or the endpoint error.
async fn find_entries(
    service: &impl RerunCloudService,
    filter: EntryFilter,
) -> tonic::Result<Vec<EntryDetails>> {
    Ok(service
        .find_entries(tonic::Request::new(FindEntriesRequest {
            filter: Some(filter),
        }))
        .await?
        .into_inner()
        .entries)
}

/// Like [`find_entries`], but panics on error (for the happy-path test bodies).
async fn find_entries_ok(
    service: &impl RerunCloudService,
    filter: EntryFilter,
) -> Vec<EntryDetails> {
    find_entries(service, filter)
        .await
        .expect("FindEntries should succeed")
}

/// A kind-less `FindEntries` must never return asset-dataset entries, while still
/// returning the dataset, the table, and blueprint entries.
///
/// This is for maintaining backwards compatibility with clients older than 0.35.
/// When 0.34 is out of support we should change this behavior to return no results.
pub async fn find_entries_default_excludes_asset_datasets(service: impl RerunCloudService) {
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let dataset = service
        .create_dataset_entry_with_name("fef_default_dataset")
        .await;
    let table = create_table_entry_with_name(&service, "fef_default_table", &tmp_dir).await;

    let entries = find_entries_ok(&service, EntryFilter::default()).await;

    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(dataset.details.id.into())),
        "expected the dataset entry in the default result set: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(table.details.id.into())),
        "expected the table entry in the default result set: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.entry_kind == EntryKind::BlueprintDataset as i32),
        "expected at least one blueprint dataset entry in the default result set: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .all(|e| e.entry_kind != EntryKind::AssetDataset as i32),
        "kind-less FindEntries must never return asset-dataset entries: {entries:?}"
    );
}

/// `entry_kinds` restricts the result set to exactly the requested kinds.
pub async fn find_entries_entry_kinds_exact(service: impl RerunCloudService) {
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let dataset = service
        .create_dataset_entry_with_name("fef_exact_dataset")
        .await;
    let table = create_table_entry_with_name(&service, "fef_exact_table", &tmp_dir).await;
    let asset_name = EntryName::asset_for(dataset.details.id);

    // `entry_kinds=[Dataset]` returns only kind-1 entries, including our dataset.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            entry_kinds: vec![EntryKind::Dataset as i32],
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries
            .iter()
            .all(|e| e.entry_kind == EntryKind::Dataset as i32),
        "entry_kinds=[Dataset] must only return Dataset entries: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(dataset.details.id.into())),
        "expected our dataset in the Dataset-only result set: {entries:?}"
    );

    let entries = find_entries_ok(
        &service,
        EntryFilter {
            entry_kinds: vec![EntryKind::AssetDataset as i32],
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries
            .iter()
            .all(|e| e.entry_kind == EntryKind::AssetDataset as i32),
        "entry_kinds=[AssetDataset] must only return AssetDataset entries: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.name.as_deref() == Some(asset_name.as_str())),
        "expected the dataset's asset entry in the AssetDataset-only result set: {entries:?}"
    );

    // `entry_kinds=[Dataset, Table]` returns exactly those two kinds, both present.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            entry_kinds: vec![EntryKind::Dataset as i32, EntryKind::Table as i32],
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries.iter().all(|e| {
            e.entry_kind == EntryKind::Dataset as i32 || e.entry_kind == EntryKind::Table as i32
        }),
        "entry_kinds=[Dataset, Table] must only return Dataset/Table entries: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(dataset.details.id.into())),
        "expected our dataset in the Dataset+Table result set: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(table.details.id.into())),
        "expected our table in the Dataset+Table result set: {entries:?}"
    );
}

/// `entry_kinds` containing `ENTRY_KIND_UNSPECIFIED` is rejected, alone or mixed with a
/// valid kind.
pub async fn find_entries_entry_kinds_rejects_unspecified(service: impl RerunCloudService) {
    let status = find_entries(
        &service,
        EntryFilter {
            entry_kinds: vec![EntryKind::Unspecified as i32],
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );

    let status = find_entries(
        &service,
        EntryFilter {
            entry_kinds: vec![EntryKind::Dataset as i32, EntryKind::Unspecified as i32],
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );
}

/// The legacy singular `entry_kind` field still works when `entry_kinds` is empty.
pub async fn find_entries_legacy_entry_kind_still_works(service: impl RerunCloudService) {
    let dataset = service
        .create_dataset_entry_with_name("fef_legacy_dataset")
        .await;

    // EXCEPTION: this test deliberately keeps using the deprecated singular `entry_kind`
    // field (rather than `entry_kinds`) to test legacy compatibility.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            entry_kind: Some(EntryKind::Dataset as i32),
            entry_kinds: vec![],
            ..Default::default()
        },
    )
    .await;

    assert!(
        entries
            .iter()
            .all(|e| e.entry_kind == EntryKind::Dataset as i32),
        "legacy entry_kind=Dataset must only return Dataset entries: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(dataset.details.id.into())),
        "expected our dataset in the legacy entry_kind=Dataset result set: {entries:?}"
    );
}

/// (Legacy compat) A name or id lookup when neither `entry_kind` or `entry_kinds` is passed matches
/// against all but `AssetDatasets`. This is odd but required for compatibility with clients older
/// than 0.35. Make the behavior more meaningful (require an `entry_kinds` or return no results)
/// when we deprecate old clients.
pub async fn find_entries_asset_by_name_requires_explicit_kind(service: impl RerunCloudService) {
    let dataset = service
        .create_dataset_entry_with_name("fef_asset_by_name_dataset")
        .await;
    let asset_name = EntryName::asset_for(dataset.details.id);

    let entries = find_entries_ok(
        &service,
        EntryFilter {
            name: Some(asset_name.to_string()),
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries.is_empty(),
        "kind-less name lookup for an asset dataset must return nothing: {entries:?}"
    );

    // Same name, with the kind requested explicitly, resolves it.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            name: Some(asset_name.to_string()),
            entry_kinds: vec![EntryKind::AssetDataset as i32],
            ..Default::default()
        },
    )
    .await;
    assert_eq!(
        entries.len(),
        1,
        "expected exactly the asset entry: {entries:?}"
    );
    assert_eq!(entries[0].name.as_deref(), Some(asset_name.as_str()));
    assert_eq!(entries[0].entry_kind, EntryKind::AssetDataset as i32);
}

/// Unknown positive `entry_kinds` values (kinds not yet known to this server version) are
/// silently ignored rather than rejected: a future kind added server-side must never break
/// an intermediate-version client that happens to send it alongside known kinds.
pub async fn find_entries_ignores_unknown_entry_kinds(service: impl RerunCloudService) {
    let dataset = service
        .create_dataset_entry_with_name("fef_unknown_kind_dataset")
        .await;

    let entries = find_entries_ok(
        &service,
        EntryFilter {
            entry_kinds: vec![EntryKind::Dataset as i32, 1000],
            ..Default::default()
        },
    )
    .await;

    assert!(
        entries
            .iter()
            .all(|e| e.entry_kind == EntryKind::Dataset as i32),
        "entry_kinds=[Dataset, 1000] must only return Dataset entries: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.id == Some(dataset.details.id.into())),
        "expected our dataset in the result set: {entries:?}"
    );
}

/// A name lookup combined with a multi-kind filter returns exactly the matching entry, with
/// no error, even though the name only matches one of the requested kinds' store families.
pub async fn find_entries_entry_kinds_multi_kind_name_lookup(service: impl RerunCloudService) {
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let table = create_table_entry_with_name(&service, "fef_multi_kind_table", &tmp_dir).await;

    // A dataset also exists (a different store family), so the name filter genuinely has to
    // miss on the dataset side and hit on the table side within the same request.
    let _dataset = service
        .create_dataset_entry_with_name("fef_multi_kind_dataset")
        .await;

    let entries = find_entries_ok(
        &service,
        EntryFilter {
            name: Some(table.details.name.to_string()),
            entry_kinds: vec![EntryKind::Dataset as i32, EntryKind::Table as i32],
            ..Default::default()
        },
    )
    .await;

    assert_eq!(
        entries.len(),
        1,
        "expected exactly the named table entry: {entries:?}"
    );
    assert_eq!(entries[0].id, Some(table.details.id.into()));
    assert_eq!(entries[0].entry_kind, EntryKind::Table as i32);
}

/// (Legacy compat) A name or id lookup when neither `entry_kind` or `entry_kinds` is passed matches
/// against all but `AssetDatasets`. This is odd but required for compatibility with clients older
/// than 0.35. Make the behavior more meaningful (require an `entry_kinds` or return no results)
/// when we deprecate old clients.
pub async fn find_entries_asset_by_id_requires_explicit_kind(service: impl RerunCloudService) {
    let dataset = service
        .create_dataset_entry_with_name("fef_asset_by_id_dataset")
        .await;
    let asset_id = dataset
        .dataset_details
        .asset_dataset
        .expect("expected an associated asset dataset");

    let entries = find_entries_ok(
        &service,
        EntryFilter {
            id: Some(asset_id.into()),
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries.is_empty(),
        "kind-less id lookup for an asset dataset must return nothing: {entries:?}"
    );

    // Same id, with the kind requested explicitly, resolves it.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            id: Some(asset_id.into()),
            entry_kinds: vec![EntryKind::AssetDataset as i32],
            ..Default::default()
        },
    )
    .await;
    assert_eq!(
        entries.len(),
        1,
        "expected exactly the asset entry: {entries:?}"
    );
    assert_eq!(entries[0].id, Some(asset_id.into()));
    assert_eq!(entries[0].entry_kind, EntryKind::AssetDataset as i32);

    // Same id, with a mismatched explicit kind, must not resolve it.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            id: Some(asset_id.into()),
            entry_kinds: vec![EntryKind::Dataset as i32],
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries.is_empty(),
        "asset id + entry_kinds=[Dataset] must not resolve the asset entry: {entries:?}"
    );
}

/// The legacy singular `entry_kind` field never surfaces `NotFound` as an error: a miss
/// (a name that doesn't exist) contributes nothing, exactly like `entry_kinds` and the
/// kind-less default.
pub async fn find_entries_legacy_entry_kind_miss_returns_empty(service: impl RerunCloudService) {
    // Deliberately exercises the deprecated `entry_kind` field: a legacy singular entry_kind
    // filter combined with a name that matches nothing must return an empty list, not error.
    let entries = find_entries_ok(
        &service,
        EntryFilter {
            entry_kind: Some(EntryKind::Dataset as i32),
            name: Some("fef_legacy_entry_kind_miss_nonexistent_name".to_owned()),
            ..Default::default()
        },
    )
    .await;
    assert!(
        entries.is_empty(),
        "a legacy entry_kind miss must return an empty list, not an error: {entries:?}"
    );
}

/// The legacy singular `entry_kind` field rejects `ENTRY_KIND_UNSPECIFIED`, exactly like
/// `entry_kinds`.
pub async fn find_entries_rejects_legacy_unspecified(service: impl RerunCloudService) {
    // Deliberately exercises the deprecated `entry_kind` field: legacy singular
    // ENTRY_KIND_UNSPECIFIED must be rejected the same way as `entry_kinds` containing it.
    let status = find_entries(
        &service,
        EntryFilter {
            entry_kind: Some(EntryKind::Unspecified as i32),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "unexpected status: {status:?}"
    );
}
