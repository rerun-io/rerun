//! Rerun Viewport Blueprint
//!
//! This crate provides blueprint (i.e. description) for how to render the viewport.

mod container;
mod space_view;
mod space_view_contents;
pub mod ui;
mod view_properties;
mod viewport_blueprint;
mod viewport_command;

pub use container::ContainerBlueprint;
use re_viewer_context::ViewerContext;
pub use space_view::SpaceViewBlueprint;
pub use space_view_contents::SpaceViewContents;
pub use view_properties::{entity_path_for_view_property, ViewProperty, ViewPropertyQueryError};
pub use viewport_blueprint::ViewportBlueprint;
pub use viewport_command::ViewportCommand;

/// The entity path of the viewport blueprint in the blueprint store.
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

/// List out all space views we generate by default for the available data.
///
/// TODO(andreas): This is transitional. We want to pass on the space view spawn heuristics
/// directly and make more high level decisions with it.
pub fn default_created_space_views(ctx: &ViewerContext<'_>) -> Vec<SpaceViewBlueprint> {
    re_tracing::profile_function!();

    ctx.space_view_class_registry
        .iter_registry()
        .flat_map(|entry| {
            let spawn_heuristics = entry.class.spawn_heuristics(ctx);
            spawn_heuristics
                .into_vec()
                .into_iter()
                .map(|recommendation| SpaceViewBlueprint::new(entry.identifier, recommendation))
        })
        .collect()
}
