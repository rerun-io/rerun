use std::collections::BTreeMap;

use itertools::Itertools;
use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, EntityPropertiesComponent, TimeInt, Timeline};
use re_log_types::{DataRow, RowId, TimePoint};

use crate::{
    SpaceViewHighlights, SpaceViewId, SystemCommand, SystemCommandSender as _, ViewSystemName,
    ViewerContext,
};

#[derive(Debug)]
pub struct DataResult {
    // TODO(jleibs): This should eventually become a more generalized (StoreView + EntityPath) reference to handle
    // multi-RRD or blueprint-static data references.
    pub entity_path: EntityPath,

    pub view_parts: Vec<ViewSystemName>,

    // TODO(jleibs): Eventually this goes away and becomes implicit as an override layer in the StoreView
    // The reason we store it here though is that context is part of the DataResult.
    pub resolved_properties: EntityProperties,

    // `EntityPath` in the Blueprint store where an override can be written
    pub override_path: EntityPath,
}

impl DataResult {
    pub fn save_override(&self, props: EntityProperties, ctx: &ViewerContext<'_>) {
        if props.has_edits(&self.resolved_properties) {
            let timepoint = TimePoint::timeless();

            let component = EntityPropertiesComponent { props };

            let row = DataRow::from_cells1_sized(
                RowId::random(),
                self.override_path.clone(),
                timepoint,
                1,
                [component],
            )
            .unwrap();

            ctx.command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    ctx.store_context.blueprint.store_id().clone(),
                    vec![row],
                ));
        }
    }
}

pub type PerSystemDataResults<'a> = BTreeMap<ViewSystemName, Vec<&'a DataResult>>;

pub struct ViewQuery<'s> {
    /// The id of the space in which context the query happens.
    pub space_view_id: SpaceViewId,

    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All queried entities.
    ///
    /// Contains also invisible objects, use `iter_entities` to iterate over visible ones.
    pub per_system_data_results: &'s PerSystemDataResults<'s>,

    /// The timeline we're on.
    pub timeline: Timeline,

    /// The time on the timeline we're currently at.
    pub latest_at: TimeInt,

    /// Hover/select highlighting information for this space view.
    ///
    /// TODO(andreas): This should be a [`crate::ViewContextSystem`] instead.
    pub highlights: &'s SpaceViewHighlights,
}

impl<'s> ViewQuery<'s> {
    /// Iter over all of the currently visible [`EntityPath`]s in the [`ViewQuery`].
    ///
    /// Also includes the corresponding [`EntityProperties`].
    pub fn iter_entities_and_properties_for_system(
        &self,
        system: ViewSystemName,
    ) -> impl Iterator<Item = (&EntityPath, &EntityProperties)> {
        self.per_system_data_results.get(&system).map_or(
            itertools::Either::Left(std::iter::empty()),
            |results| {
                itertools::Either::Right(
                    results
                        .iter()
                        .map(|data_result| {
                            (&data_result.entity_path, &data_result.resolved_properties)
                        })
                        .filter(|(_, props)| props.visible),
                )
            },
        )
    }

    /// Iterates over all [`DataResult`]s of the [`ViewQuery`].
    pub fn iter_data_results(&self) -> impl Iterator<Item = &DataResult> + '_ {
        self.per_system_data_results
            .values()
            .flat_map(|data_results| data_results.iter().copied())
    }

    /// Iterates over all entities of the [`ViewQuery`].
    pub fn iter_entities(&self) -> impl Iterator<Item = &EntityPath> + '_ {
        self.iter_data_results()
            .map(|data_result| &data_result.entity_path)
            .unique()
    }

    pub fn latest_at_query(&self) -> LatestAtQuery {
        LatestAtQuery {
            timeline: self.timeline,
            at: self.latest_at,
        }
    }
}
