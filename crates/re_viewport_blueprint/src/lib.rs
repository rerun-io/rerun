mod data_query;
mod space_view;
mod space_view_contents;
mod view_properties; // TODO(andreas): better name before `sub_archetype` sticks around?

pub use data_query::{DataQuery, EntityOverrideContext, PropertyResolver};
pub use space_view::SpaceViewBlueprint;
pub use space_view_contents::SpaceViewContents;
pub use view_properties::{
    edit_blueprint_component, entity_path_for_view_property, get_blueprint_component,
    query_view_property, query_view_property_or_default, view_property,
};
