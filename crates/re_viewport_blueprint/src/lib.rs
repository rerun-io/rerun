//! Rerun Viewport Blueprint
//!
//! This crate provides blueprint (i.e. description) for how to render the viewport.

mod container;
mod space_view;
mod space_view_contents;
mod tree_actions;
pub mod ui;
mod view_properties;
mod viewport_blueprint;

pub use container::ContainerBlueprint;
pub use space_view::SpaceViewBlueprint;
pub use space_view_contents::SpaceViewContents;
pub use tree_actions::TreeAction;
pub use view_properties::{
    edit_blueprint_component, entity_path_for_view_property, get_blueprint_component,
    query_view_property, query_view_property_or_default, view_property,
};
pub use viewport_blueprint::ViewportBlueprint;

pub const VIEWPORT_PATH: &str = "viewport";

/// Converts a [`re_types_blueprint::blueprint::components::ContainerKind`] into a [`egui_tiles::ContainerKind`].
///
/// Does not implement the `From`/`To` traits because we don't want `re_types_blueprint` to depend
/// on `egui`, and we cannot do it from here because of orphan rules.
#[inline]
pub fn container_kind_to_egui(
    kind: re_types_blueprint::blueprint::components::ContainerKind,
) -> egui_tiles::ContainerKind {
    use re_types_blueprint::blueprint::components::ContainerKind;
    match kind {
        ContainerKind::Tabs => egui_tiles::ContainerKind::Tabs,
        ContainerKind::Horizontal => egui_tiles::ContainerKind::Horizontal,
        ContainerKind::Vertical => egui_tiles::ContainerKind::Vertical,
        ContainerKind::Grid => egui_tiles::ContainerKind::Grid,
    }
}

/// Converts a [`egui_tiles::ContainerKind`] into a [`re_types_blueprint::blueprint::components::ContainerKind`].
///
/// Does not implement the `From`/`To` traits because we don't want `re_types_blueprint` to depend
/// on `egui`, and we cannot do it from here because of orphan rules.
#[inline]
pub fn container_kind_from_egui(
    kind: egui_tiles::ContainerKind,
) -> re_types_blueprint::blueprint::components::ContainerKind {
    use re_types_blueprint::blueprint::components::ContainerKind;
    match kind {
        egui_tiles::ContainerKind::Tabs => ContainerKind::Tabs,
        egui_tiles::ContainerKind::Horizontal => ContainerKind::Horizontal,
        egui_tiles::ContainerKind::Vertical => ContainerKind::Vertical,
        egui_tiles::ContainerKind::Grid => ContainerKind::Grid,
    }
}
