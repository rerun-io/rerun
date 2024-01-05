use re_data_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPropertiesComponent};
use re_log_types::Timeline;
use re_types::blueprint::components::{
    ActiveTab, ColumnShares, EntitiesDeterminedByUser, IncludedContents, IncludedQueries, Name,
    RowShares, SpaceViewClass, SpaceViewOrigin, Visible,
};
use re_types_core::Component;
use re_viewport::{
    blueprint::components::{
        AutoLayout, AutoSpaceViews, ContainerKind, GridColumns, IncludedSpaceViews, RootContainer,
        SpaceViewMaximized, ViewportLayout,
    },
    external::re_space_view::blueprint::components::QueryExpressions,
};

use crate::blueprint::components::PanelView;

fn validate_component<C: Component>(blueprint: &EntityDb) -> bool {
    let query = LatestAtQuery::latest(Timeline::default());

    if let Some(data_type) = blueprint.data_store().lookup_datatype(&C::name()) {
        if data_type != &C::arrow_datatype() {
            // If the schemas don't match, we definitely have a problem
            re_log::debug!(
                "Unexpected datatype for component {:?}.\nFound: {:#?}\nExpected: {:#?}",
                C::name(),
                data_type,
                C::arrow_datatype()
            );
            return false;
        } else {
            // Otherwise, our usage of serde-fields means we still might have a problem
            // this can go away once we stop using serde-fields.
            // Walk the blueprint and see if any cells fail to deserialize for this component type.
            for path in blueprint.entity_paths() {
                if let Some([Some(cell)]) = blueprint
                    .data_store()
                    .latest_at(&query, path, C::name(), &[C::name()])
                    .map(|(_, cells)| cells)
                {
                    if let Err(err) = cell.try_to_native_mono::<C>() {
                        re_log::debug!(
                            "Failed to deserialize component {:?}: {:?}",
                            C::name(),
                            err
                        );
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Because blueprints are both read and written the schema must match what
/// we expect to find or else we will run into all kinds of problems.
pub fn is_valid_blueprint(blueprint: &EntityDb) -> bool {
    // TODO(#4708): Generate this from codegen.
    validate_component::<AutoSpaceViews>(blueprint)
        && validate_component::<ActiveTab>(blueprint)
        && validate_component::<AutoLayout>(blueprint)
        && validate_component::<AutoSpaceViews>(blueprint)
        && validate_component::<ColumnShares>(blueprint)
        && validate_component::<ContainerKind>(blueprint)
        && validate_component::<EntitiesDeterminedByUser>(blueprint)
        && validate_component::<EntityPropertiesComponent>(blueprint)
        && validate_component::<GridColumns>(blueprint)
        && validate_component::<IncludedContents>(blueprint)
        && validate_component::<IncludedQueries>(blueprint)
        && validate_component::<IncludedSpaceViews>(blueprint)
        && validate_component::<Name>(blueprint)
        && validate_component::<PanelView>(blueprint)
        && validate_component::<QueryExpressions>(blueprint)
        && validate_component::<RootContainer>(blueprint)
        && validate_component::<RowShares>(blueprint)
        && validate_component::<SpaceViewClass>(blueprint)
        && validate_component::<SpaceViewMaximized>(blueprint)
        && validate_component::<SpaceViewOrigin>(blueprint)
        && validate_component::<SpaceViewClass>(blueprint)
        && validate_component::<ViewportLayout>(blueprint)
        && validate_component::<Visible>(blueprint)
}
