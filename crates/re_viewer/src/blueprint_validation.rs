use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPropertiesComponent, StoreDb};
use re_log_types::Timeline;
use re_types::blueprint::SpaceViewComponent;
use re_types_core::Component;
use re_viewport::{
    blueprint::{AutoSpaceViews, SpaceViewMaximized, ViewportLayout},
    external::re_space_view::QueryExpressions,
};

use crate::blueprint::PanelView;

fn validate_component<C: Component>(blueprint: &StoreDb) -> bool {
    let query = LatestAtQuery::latest(Timeline::default());

    if let Some(data_type) = blueprint.entity_db().data_store.lookup_datatype(&C::name()) {
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
            for path in blueprint.entity_db().entity_paths() {
                if let Some([Some(cell)]) = blueprint
                    .entity_db()
                    .data_store
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
pub fn is_valid_blueprint(blueprint: &StoreDb) -> bool {
    // TODO(jleibs): Generate this from codegen.
    validate_component::<AutoSpaceViews>(blueprint)
        && validate_component::<EntityPropertiesComponent>(blueprint)
        && validate_component::<PanelView>(blueprint)
        && validate_component::<QueryExpressions>(blueprint)
        && validate_component::<SpaceViewComponent>(blueprint)
        && validate_component::<SpaceViewMaximized>(blueprint)
        && validate_component::<ViewportLayout>(blueprint)
}
