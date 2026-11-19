use re_catalog_client::{BlueprintKind, RegisteredBlueprint};
use re_log_types::EntryId;
use re_log_types::external::re_types_core::SegmentId;
use re_redap_client::ApiError;
use re_ui::ServerValue;

use crate::entry_meta::BlueprintsRef;

/// The blueprints registered for a catalog dataset.
///
/// Each blueprint is a segment of the dataset's blueprint dataset.
#[derive(Clone, Debug)]
pub struct DatasetBlueprints {
    /// The catalog server the dataset is on.
    pub origin: re_uri::Origin,

    /// The dataset that holds the blueprints.
    pub blueprint_dataset: EntryId,

    /// All blueprints of the dataset, as far as the server has listed them, table blueprints
    /// included.
    pub blueprints: ServerValue<BlueprintsRef, ApiError>,

    /// The blueprint that recordings of the dataset open with.
    pub default_segment: Option<SegmentId>,
}

impl DatasetBlueprints {
    /// The recording blueprints, which are the ones a recording of the dataset can open with.
    ///
    /// A blueprint marked as a table blueprint is left out, and so is a blueprint with no stored
    /// kind. The dataset's default blueprint is always kept.
    ///
    /// `None` until the server has listed the blueprints.
    pub fn recording_blueprints(&self) -> Option<impl Iterator<Item = &RegisteredBlueprint>> {
        let blueprints = self.blueprints.get()?;
        Some(blueprints.iter().filter(|blueprint| {
            blueprint.kind == Some(BlueprintKind::Recording)
                || self.default_segment.as_ref() == Some(&blueprint.segment_id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_catalog_client::BlueprintName;

    use super::*;

    /// Table blueprints and blueprints with no stored kind are left out of the recording
    /// blueprints, except for the dataset's default blueprint.
    #[test]
    fn recording_blueprints_leave_out_other_kinds() {
        let blueprint = |id: &str, kind| RegisteredBlueprint {
            segment_id: SegmentId::from(id),
            name: Some(BlueprintName::try_new(id).expect("non-empty name")),
            kind,
            registration_time: None,
        };
        let dataset_blueprints = DatasetBlueprints {
            origin: re_uri::Origin::test(),
            blueprint_dataset: EntryId::new(),
            blueprints: ServerValue::Completed(Arc::new(vec![
                blueprint("recording", Some(BlueprintKind::Recording)),
                blueprint("table", Some(BlueprintKind::Table)),
                blueprint("no_kind", None),
            ])),
            default_segment: Some(SegmentId::from("table")),
        };

        let segment_ids = dataset_blueprints
            .recording_blueprints()
            .expect("the blueprints are listed")
            .map(|blueprint| blueprint.segment_id.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            segment_ids,
            [SegmentId::from("recording"), SegmentId::from("table")]
        );
    }
}
