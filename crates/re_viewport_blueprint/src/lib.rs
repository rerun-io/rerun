//! Blueprint integration for the central viewport panel of the Rerun viewer.

pub const VIEWPORT_PATH: &str = "viewport";

mod container;
mod tree_action;
mod viewport_blueprint;
mod viewport_blueprint_ui;

pub use self::container::ContainerBlueprint;
pub use self::tree_action::TreeAction;
pub use self::viewport_blueprint::ViewportBlueprint;
pub use self::viewport_blueprint_ui::contents_name_style;

pub mod external {
    pub use re_types_blueprint;
}

// ---

/// Determines the icon to use for a given container kind.
#[inline]
pub fn icon_for_container_kind(kind: &egui_tiles::ContainerKind) -> &'static re_ui::Icon {
    match kind {
        egui_tiles::ContainerKind::Tabs => &re_ui::icons::CONTAINER_TABS,
        egui_tiles::ContainerKind::Horizontal => &re_ui::icons::CONTAINER_HORIZONTAL,
        egui_tiles::ContainerKind::Vertical => &re_ui::icons::CONTAINER_VERTICAL,
        egui_tiles::ContainerKind::Grid => &re_ui::icons::CONTAINER_GRID,
    }
}

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
