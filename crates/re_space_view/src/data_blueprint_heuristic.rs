use re_viewer_context::ViewerContext;

use crate::DataBlueprintTree;

/// Trait implemented by a [`re_viewer_context::SpaceViewState`] to update the data blueprint tree.
///
/// TODO(wumpf/jleibs): This is an interim construct until we're able to extract the data blueprint via a query
///                     and figure out how default/heuristically determined values are handled.
pub trait DataBlueprintHeuristic {
    fn update_object_property_heuristics(
        &self,
        ctx: &mut ViewerContext<'_>,
        data_blueprint: &mut DataBlueprintTree,
    );
}
