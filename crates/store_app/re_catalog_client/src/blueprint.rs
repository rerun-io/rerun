//! Blueprints registered in a blueprint dataset, and their names.

use std::collections::BTreeMap;
use std::time::Duration;

use arrow::array::RecordBatch;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    BlueprintKind, DataSource, DatasetEntry, StorageLocationAlias, TableEntry, read_blueprint_kind,
    read_blueprint_name, with_blueprint_kind, with_blueprint_name, without_blueprint_name,
};
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_redap_client::{ApiError, ApiErrorKind, ApiResult};

use crate::CatalogClient;

/// The name of a blueprint, which is never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlueprintName(String);

/// Error returned when constructing a [`BlueprintName`] from an empty string.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("A blueprint name can't be empty")]
pub struct InvalidBlueprintNameError;

impl BlueprintName {
    /// Fails with [`InvalidBlueprintNameError`] if `name` is empty.
    pub fn try_new(name: impl Into<String>) -> Result<Self, InvalidBlueprintNameError> {
        let name = name.into();
        if name.is_empty() {
            return Err(InvalidBlueprintNameError);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for BlueprintName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A blueprint registered in a blueprint dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredBlueprint {
    /// The segment of the blueprint in its blueprint dataset, which is also the blueprint's ID.
    pub segment_id: SegmentId,

    /// The stored name of the blueprint, unique within its blueprint dataset.
    ///
    /// `None` if the blueprint has no stored name.
    pub name: Option<BlueprintName>,

    /// Whether this is a table blueprint or a recording blueprint.
    pub kind: BlueprintKind,
}

impl RegisteredBlueprint {
    /// The blueprint with this segment ID, with the name and kind from its entry in `properties`.
    fn new(segment_id: SegmentId, properties: &BTreeMap<SegmentId, (RecordBatch, u64)>) -> Self {
        let properties = properties
            .get(&segment_id)
            .map(|(properties, _)| properties);
        Self::from_properties(segment_id, properties)
    }

    /// The blueprint with this segment ID, with the name and kind from its single-row
    /// `properties`.
    fn from_properties(segment_id: SegmentId, properties: Option<&RecordBatch>) -> Self {
        let name = properties
            .and_then(|properties| read_blueprint_name(properties, 0))
            .and_then(|name| BlueprintName::try_new(name).ok());
        let kind = properties
            .map(|properties| read_blueprint_kind(properties, 0))
            .unwrap_or_default();
        Self {
            segment_id,
            name,
            kind,
        }
    }
}

impl CatalogClient {
    /// The blueprints of a blueprint dataset, in the order of its segment table.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_blueprints(
        &self,
        blueprint_dataset: EntryId,
    ) -> ApiResult<Vec<RegisteredBlueprint>> {
        let segment_ids = self.get_dataset_segment_ids(blueprint_dataset).await?;
        let properties = self
            .blueprint_properties(blueprint_dataset, segment_ids.clone())
            .await?;

        Ok(segment_ids
            .into_iter()
            .map(|segment_id| RegisteredBlueprint::new(segment_id, &properties))
            .collect())
    }

    /// The blueprint with this name, or `None` if no blueprint of the dataset has it.
    ///
    /// Never returns a blueprint without a stored name.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_blueprint_by_name(
        &self,
        blueprint_dataset: EntryId,
        name: &BlueprintName,
    ) -> ApiResult<Option<RegisteredBlueprint>> {
        Ok(self
            .get_blueprints(blueprint_dataset)
            .await?
            .into_iter()
            .find(|blueprint| blueprint.name.as_ref() == Some(name)))
    }

    /// Register an existing `.rbl` that the server can reach as a `kind` blueprint of
    /// `blueprint_dataset`, and wait for the registration to finish.
    ///
    /// With a `name`, registering a blueprint that is already registered first sets its name and
    /// kind, and then replaces its data.
    ///
    /// Fails with `AlreadyExists` if a blueprint with another segment ID has this name. An already
    /// registered blueprint keeps its data when naming fails, and a blueprint this call added is
    /// unregistered again. If replacing the data fails, the blueprint keeps its old data under the
    /// new name.
    ///
    /// Without a `name`, a new blueprint has no name, even if an earlier registration of the same
    /// `.rbl` had one. Registering a blueprint that is already registered sets its kind, replaces
    /// its data, and keeps the name stored for it. Logs a warning once per process when the
    /// registered blueprint has no name.
    ///
    /// On a server that doesn't store blueprint names, `name` and `kind` are ignored with a
    /// warning, and the returned blueprint is a recording blueprint without a name, the same as in
    /// [`Self::get_blueprints`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_blueprint(
        &self,
        blueprint_dataset: EntryId,
        blueprint_uri: impl AsRef<str>,
        name: Option<&BlueprintName>,
        kind: BlueprintKind,
        storage_location: Option<StorageLocationAlias>,
        timeout: Duration,
    ) -> ApiResult<RegisteredBlueprint> {
        let data_source = DataSource::new_rrd(blueprint_uri, storage_location).map_err(|err| {
            ApiError::invalid_arguments_with_source(
                self.origin(),
                None,
                err,
                "invalid blueprint url",
            )
        })?;

        let existing = self.get_dataset_segment_ids(blueprint_dataset).await?;

        let Some(name) = name else {
            let segment_id = self
                .register_blueprint_source(
                    blueprint_dataset,
                    data_source,
                    IfDuplicateBehavior::Overwrite,
                    timeout,
                )
                .await?;
            let mut properties = self
                .blueprint_properties(blueprint_dataset, vec![segment_id.clone()])
                .await?;
            let stored = properties.remove(&segment_id);

            // Segment properties outlive unregistering, so a new blueprint can come with the name
            // of an earlier registration of the same segment.
            let clear_name = !existing.contains(&segment_id);

            let blueprint = match self
                .write_unnamed_blueprint_properties(
                    blueprint_dataset,
                    segment_id.clone(),
                    stored.as_ref(),
                    clear_name,
                    kind,
                )
                .await
            {
                Ok(blueprint) => blueprint,

                Err(err) if err.kind == ApiErrorKind::Unimplemented => {
                    if kind != BlueprintKind::default() {
                        re_log::warn!(
                            "The server doesn't store blueprint properties, so the blueprint kind is ignored"
                        );
                    }
                    RegisteredBlueprint::from_properties(
                        segment_id,
                        stored.as_ref().map(|(properties, _)| properties),
                    )
                }

                Err(err) => return Err(err),
            };

            if blueprint.name.is_none() {
                warn_unnamed_blueprint();
            }
            return Ok(blueprint);
        };

        // `Skip` leaves an already registered blueprint unchanged, and returns its segment ID.
        let segment_id = self
            .register_blueprint_source(
                blueprint_dataset,
                data_source.clone(),
                IfDuplicateBehavior::Skip,
                timeout,
            )
            .await?;

        let added = !existing.contains(&segment_id);

        let blueprint = match self
            .write_blueprint_properties(blueprint_dataset, segment_id.clone(), name, Some(kind))
            .await
        {
            Ok(blueprint) => blueprint,

            Err(err) if err.kind == ApiErrorKind::Unimplemented => {
                re_log::warn!(
                    "The server doesn't store blueprint names, so the name {:?} and the blueprint kind are ignored",
                    name.as_str()
                );
                RegisteredBlueprint::from_properties(segment_id.clone(), None)
            }

            Err(err) => {
                if added
                    && let Err(cleanup_err) = self
                        .unregister_blueprint_and_wait(blueprint_dataset, segment_id, timeout)
                        .await
                {
                    re_log::warn!(
                        "Failed to unregister the blueprint after naming it failed: {cleanup_err}"
                    );
                }
                return Err(err);
            }
        };

        // An `Overwrite` registration keeps the name stored for the segment.
        if !added {
            self.register_blueprint_source(
                blueprint_dataset,
                data_source,
                IfDuplicateBehavior::Overwrite,
                timeout,
            )
            .await?;
        }

        Ok(blueprint)
    }

    /// Register one `.rbl` with a blueprint dataset, wait for the registration to finish, and
    /// return its segment ID.
    async fn register_blueprint_source(
        &self,
        blueprint_dataset: EntryId,
        data_source: DataSource,
        on_duplicate: IfDuplicateBehavior,
        timeout: Duration,
    ) -> ApiResult<SegmentId> {
        self.register_with_dataset(blueprint_dataset, vec![data_source], on_duplicate)
            .await?
            .wait(timeout)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                ApiError::internal(
                    self.origin(),
                    "the server registered the blueprint but returned no segment",
                )
            })
    }

    /// Unregister a blueprint and wait for the unregistration to finish.
    async fn unregister_blueprint_and_wait(
        &self,
        blueprint_dataset: EntryId,
        segment_id: SegmentId,
        timeout: Duration,
    ) -> ApiResult {
        let (trace_id, task_ids) = self
            .unregister_from_dataset(blueprint_dataset, vec![segment_id], Vec::new(), false)
            .await?;
        self.client()
            .await?
            .wait_for_tasks(task_ids, trace_id, timeout)
            .await
    }

    /// The default `kind` blueprint of a dataset, or `None` if it has none.
    ///
    /// The recordings of the dataset open with the default recording blueprint, and its segment
    /// table with the default table blueprint.
    ///
    /// Doesn't check that the blueprint dataset has a blueprint with the default segment ID, so a
    /// segment ID it doesn't have comes back as a blueprint without a name.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_default_blueprint(
        &self,
        dataset_id: EntryId,
        kind: BlueprintKind,
    ) -> ApiResult<Option<RegisteredBlueprint>> {
        let details = self.read_dataset(dataset_id).await?.dataset_details;
        let segment_id = match kind {
            BlueprintKind::Recording => details.default_blueprint_segment,
            BlueprintKind::Table => details.default_segment_table_blueprint_segment,
        };
        self.default_blueprint(details.blueprint_dataset, segment_id)
            .await
    }

    /// The default blueprint of a table, or `None` if it has none.
    ///
    /// Doesn't check that the blueprint dataset has a blueprint with the default segment ID, so a
    /// segment ID it doesn't have comes back as a blueprint without a name.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_table_default_blueprint(
        &self,
        table_id: EntryId,
    ) -> ApiResult<Option<RegisteredBlueprint>> {
        let details = self.read_table(table_id).await?.table_details;
        self.default_blueprint(details.blueprint_dataset, details.default_blueprint_segment)
            .await
    }

    async fn default_blueprint(
        &self,
        blueprint_dataset: Option<EntryId>,
        segment_id: Option<SegmentId>,
    ) -> ApiResult<Option<RegisteredBlueprint>> {
        let (Some(blueprint_dataset), Some(segment_id)) = (blueprint_dataset, segment_id) else {
            return Ok(None);
        };

        let properties = self
            .blueprint_properties(blueprint_dataset, vec![segment_id.clone()])
            .await?;

        Ok(Some(RegisteredBlueprint::new(segment_id, &properties)))
    }

    /// Set the default `kind` blueprint of a dataset to the blueprint with this segment ID.
    /// `None` clears it.
    ///
    /// Returns the dataset as the server stores it after the update.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn set_dataset_default_blueprint(
        &self,
        dataset_id: EntryId,
        kind: BlueprintKind,
        segment_id: Option<SegmentId>,
    ) -> ApiResult<DatasetEntry> {
        let mut details = self.read_dataset(dataset_id).await?.dataset_details;
        match kind {
            BlueprintKind::Recording => details.default_blueprint_segment = segment_id,
            BlueprintKind::Table => details.default_segment_table_blueprint_segment = segment_id,
        }
        self.update_dataset(dataset_id, details).await
    }

    /// Set the default blueprint of a table to the blueprint with this segment ID, or clear it
    /// with `None`.
    ///
    /// Updating a table that has no blueprint dataset yet creates one.
    ///
    /// Returns the table as the server stores it after the update.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn set_table_default_blueprint(
        &self,
        table_id: EntryId,
        segment_id: Option<SegmentId>,
    ) -> ApiResult<TableEntry> {
        let mut details = self.read_table(table_id).await?.table_details;
        details.default_blueprint_segment = segment_id;
        self.update_table(table_id, details).await
    }

    /// Name a blueprint of a blueprint dataset, and return the renamed blueprint.
    ///
    /// Fails with `NotFound` if the dataset has no blueprint with this segment ID, and with
    /// `AlreadyExists` if another blueprint of the dataset has this name. A blueprint can be given
    /// the name it already has. Fails with `Unimplemented` on a server that doesn't store
    /// blueprint names.
    ///
    /// Two clients naming different blueprints the same at the same time can both succeed, since
    /// the server checks the revision of the renamed blueprint only. The write fails with
    /// `FailedPrecondition` if the blueprint itself changes between the check and the write.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn set_blueprint_name(
        &self,
        blueprint_dataset: EntryId,
        segment_id: SegmentId,
        name: &BlueprintName,
    ) -> ApiResult<RegisteredBlueprint> {
        self.write_blueprint_properties(blueprint_dataset, segment_id, name, None)
            .await
    }

    /// Name a blueprint like [`Self::set_blueprint_name`], and set its kind if `kind` is `Some`.
    /// The other stored properties of the blueprint stay the same.
    async fn write_blueprint_properties(
        &self,
        blueprint_dataset: EntryId,
        segment_id: SegmentId,
        name: &BlueprintName,
        kind: Option<BlueprintKind>,
    ) -> ApiResult<RegisteredBlueprint> {
        let segment_ids = self.get_dataset_segment_ids(blueprint_dataset).await?;
        if !segment_ids.contains(&segment_id) {
            return Err(ApiError::not_found(
                self.origin(),
                re_error::format_with_details(
                    "The blueprint dataset has no blueprint with this segment ID",
                    format!("Blueprint: {segment_id}"),
                ),
            ));
        }

        let mut properties = self
            .client()
            .await?
            .get_segment_properties(blueprint_dataset, segment_ids.clone())
            .await
            .map_err(|err| names_unsupported_error(self.origin(), err))?;

        let taken_by = segment_ids.into_iter().find(|other| {
            *other != segment_id
                && RegisteredBlueprint::new(other.clone(), &properties)
                    .name
                    .as_ref()
                    == Some(name)
        });
        if let Some(other) = taken_by {
            return Err(name_taken_error(self.origin(), name, &other));
        }

        let stored = properties.remove(&segment_id);
        let revision = stored.as_ref().map_or(0, |(_, revision)| *revision);
        let properties =
            with_blueprint_name(stored.as_ref().map(|(batch, _)| batch), name.as_str())
                .and_then(|properties| match kind {
                    Some(kind) => with_blueprint_kind(Some(&properties), kind),
                    None => Ok(properties),
                })
                .map_err(|err| {
                    ApiError::serialization_with_source(
                        self.origin(),
                        err,
                        "failed building the blueprint property",
                    )
                })?;

        self.set_segment_properties(
            blueprint_dataset,
            segment_id.clone(),
            &properties,
            Some(revision),
        )
        .await
        .map_err(|err| names_unsupported_error(self.origin(), err))?;

        Ok(RegisteredBlueprint::from_properties(
            segment_id,
            Some(&properties),
        ))
    }

    /// Set the kind of a blueprint without naming it, and clear its stored name if `clear_name`.
    /// The other stored properties of the blueprint stay the same.
    ///
    /// `stored` are the blueprint's stored properties and their revision, if it has any. Fails
    /// with `FailedPrecondition` if the stored properties changed since they were read.
    async fn write_unnamed_blueprint_properties(
        &self,
        blueprint_dataset: EntryId,
        segment_id: SegmentId,
        stored: Option<&(RecordBatch, u64)>,
        clear_name: bool,
        kind: BlueprintKind,
    ) -> ApiResult<RegisteredBlueprint> {
        let revision = stored.map_or(0, |(_, revision)| *revision);
        let properties = with_blueprint_kind(stored.map(|(batch, _)| batch), kind)
            .and_then(|properties| {
                if clear_name {
                    without_blueprint_name(&properties)
                } else {
                    Ok(properties)
                }
            })
            .map_err(|err| {
                ApiError::serialization_with_source(
                    self.origin(),
                    err,
                    "failed building the blueprint property",
                )
            })?;

        self.set_segment_properties(
            blueprint_dataset,
            segment_id.clone(),
            &properties,
            Some(revision),
        )
        .await
        .map_err(|err| names_unsupported_error(self.origin(), err))?;

        Ok(RegisteredBlueprint::from_properties(
            segment_id,
            Some(&properties),
        ))
    }

    /// The stored properties of the given blueprints.
    ///
    /// Empty on a server without segment properties.
    async fn blueprint_properties(
        &self,
        blueprint_dataset: EntryId,
        segment_ids: Vec<SegmentId>,
    ) -> ApiResult<BTreeMap<SegmentId, (RecordBatch, u64)>> {
        let result = self
            .client()
            .await?
            .get_segment_properties(blueprint_dataset, segment_ids)
            .await;

        match result {
            Ok(properties) => Ok(properties),

            Err(err) if err.kind == ApiErrorKind::Unimplemented => {
                re_log::debug_once!("The server does not store blueprint names");
                Ok(BTreeMap::new())
            }

            Err(err) => Err(err),
        }
    }
}

/// Logs a warning about a blueprint registered without a name, once per process.
fn warn_unnamed_blueprint() {
    re_log::warn_once!(
        "A blueprint was registered without a name. Give it a name when registering it, or rename it afterwards."
    );
}

/// Wraps an `Unimplemented` error from the segment properties endpoints in one that says the
/// server doesn't store blueprint names. Other errors come back unchanged.
fn names_unsupported_error(origin: &re_uri::Origin, err: ApiError) -> ApiError {
    if err.kind == ApiErrorKind::Unimplemented {
        ApiError::with_kind_and_source(
            origin,
            ApiErrorKind::Unimplemented,
            None,
            err,
            "The server doesn't store blueprint names",
        )
    } else {
        err
    }
}

fn name_taken_error(
    origin: &re_uri::Origin,
    name: &BlueprintName,
    taken_by: &SegmentId,
) -> ApiError {
    ApiError::already_exists(
        origin,
        re_error::format_with_details(
            "Another blueprint already has this name",
            format!("Name: {name}\nBlueprint: {taken_by}"),
        ),
    )
}
