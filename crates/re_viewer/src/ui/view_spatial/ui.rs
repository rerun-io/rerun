use re_data_store::ObjPath;
use re_format::format_f32;

use egui::{NumExt, WidgetText};
use macaw::BoundingBox;

use crate::misc::{space_info::SpaceInfo, ViewerContext};

use super::{ui_2d::View2DState, ui_3d::View3DState, SceneSpatial, SpaceSpecs};

/// Describes how the scene is navigated, determining if it is a 2D or 3D experience.
#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SpatialNavigationMode {
    #[default]
    TwoD,
    ThreeD,
}

impl From<SpatialNavigationMode> for WidgetText {
    fn from(val: SpatialNavigationMode) -> Self {
        match val {
            SpatialNavigationMode::TwoD => "2D Pan & Zoom".into(),
            SpatialNavigationMode::ThreeD => "3D Camera".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AutoSizeUnit {
    Auto,
    UiPoints,
    World,
}
impl From<AutoSizeUnit> for WidgetText {
    fn from(val: AutoSizeUnit) -> Self {
        match val {
            AutoSizeUnit::Auto => "Auto".into(),
            AutoSizeUnit::UiPoints => "UI points".into(),
            AutoSizeUnit::World => "Scene units".into(),
        }
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct ViewSpatialState {
    /// How the scene is navigated.
    pub nav_mode: SpatialNavigationMode,

    /// Estimated bounding box of all data. Accumulated over every time data is displayed.
    ///
    /// Specify default explicitly, otherwise it will be a box at 0.0 after deserialization.
    #[serde(skip, default = "default_scene_bbox_accum")]
    pub scene_bbox_accum: BoundingBox,
    /// Estimated number of primitives last frame. Used to inform some heuristics.
    #[serde(skip)]
    pub scene_num_primitives: usize,

    pub(super) state_2d: View2DState,
    pub(super) state_3d: View3DState,

    /// Size of automatically sized objects. None if it wasn't configured.
    auto_size_config: re_renderer::AutoSizeConfig,
}

fn default_scene_bbox_accum() -> BoundingBox {
    BoundingBox::nothing()
}

impl Default for ViewSpatialState {
    fn default() -> Self {
        Self {
            nav_mode: Default::default(),
            scene_bbox_accum: default_scene_bbox_accum(),
            scene_num_primitives: 0,
            state_2d: Default::default(),
            state_3d: Default::default(),
            auto_size_config: re_renderer::AutoSizeConfig {
                point_radius: re_renderer::Size::AUTO, // let re_renderer decide
                line_radius: re_renderer::Size::AUTO,  // let re_renderer decide
            },
        }
    }
}

impl ViewSpatialState {
    pub fn auto_size_config(
        &self,
        viewport_size_in_points: egui::Vec2,
    ) -> re_renderer::AutoSizeConfig {
        let mut config = self.auto_size_config;
        if config.point_radius.is_auto() {
            config.point_radius = self.default_point_radius(viewport_size_in_points);
        }
        if config.line_radius.is_auto() {
            config.line_radius = self.default_line_radius();
        }
        config
    }

    #[allow(clippy::unused_self)]
    pub fn default_line_radius(&self) -> re_renderer::Size {
        re_renderer::Size::new_points(1.5)
    }

    pub fn default_point_radius(&self, viewport_size_in_points: egui::Vec2) -> re_renderer::Size {
        let num_points = self.scene_num_primitives; // approximately the same thing when there are many points

        // Larger view -> larger points.
        let viewport_area = viewport_size_in_points.x * viewport_size_in_points.y;

        // More points -> smaller points.
        let radius = (0.3 * (viewport_area / (num_points + 1) as f32).sqrt()).clamp(0.2, 5.0);

        re_renderer::Size::new_points(radius)
    }

    fn auto_size_world_heuristic(&self) -> f32 {
        if self.scene_bbox_accum.is_nothing() || self.scene_bbox_accum.is_nan() {
            return 0.01;
        }

        // Motivation: Size should be proportional to the scene extent, here covered by its diagonal
        let diagonal_length = (self.scene_bbox_accum.max - self.scene_bbox_accum.min).length();
        let heuristic0 = diagonal_length * 0.0025;

        // Motivation: A lot of times we look at the entire scene and expect to see everything on a flat screen with some gaps between.
        let size = self.scene_bbox_accum.size();
        let mut sorted_components = size.to_array();
        sorted_components.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Median is more robust against outlier in one direction (as such pretty pour still though)
        let median_extent = sorted_components[1];
        // sqrt would make more sense but using a smaller root works better in practice.
        let heuristic1 =
            (median_extent / (self.scene_num_primitives.at_least(1) as f32).powf(1.0 / 1.7)) * 0.25;

        heuristic0.min(heuristic1)
    }

    pub fn settings_ui(&mut self, _ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        egui::Grid::new("spatial_settings_ui").show(ui, |ui| {
            let auto_size_world = self.auto_size_world_heuristic();

            ui.label("Default point radius:")
                .on_hover_text("Point radius used whenever not explicitly specified.");
            ui.push_id("points", |ui| {
                size_ui(
                    ui,
                    2.0,
                    auto_size_world,
                    &mut self.auto_size_config.point_radius,
                );
            });
            ui.end_row();

            ui.label("Default line radius:")
                .on_hover_text("Line radius used whenever not explicitly specified.");
            ui.push_id("lines", |ui| {
                size_ui(
                    ui,
                    1.5,
                    auto_size_world,
                    &mut self.auto_size_config.line_radius,
                );
            });
            ui.end_row();

            ui.label("Navigation mode:");
            egui::ComboBox::from_id_source("nav_mode")
                .selected_text(self.nav_mode)
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.selectable_value(
                        &mut self.nav_mode,
                        SpatialNavigationMode::TwoD,
                        SpatialNavigationMode::TwoD,
                    );
                    ui.selectable_value(
                        &mut self.nav_mode,
                        SpatialNavigationMode::ThreeD,
                        SpatialNavigationMode::ThreeD,
                    );
                });
            ui.end_row();
        });

        ui.separator();

        let BoundingBox { min, max } = self.scene_bbox_accum;

        match self.nav_mode {
            SpatialNavigationMode::TwoD => {
                ui.label(format!(
                    "Bounding box:\n  x: [{} - {}]\n  y: [{} - {}]",
                    format_f32(min.x),
                    format_f32(max.x),
                    format_f32(min.y),
                    format_f32(max.y),
                ));
            }
            SpatialNavigationMode::ThreeD => {
                ui.label(format!(
                    "Bounding box:\n  x: [{} - {}]\n  y: [{} - {}]\n  z: [{} - {}]",
                    format_f32(min.x),
                    format_f32(max.x),
                    format_f32(min.y),
                    format_f32(max.y),
                    format_f32(min.z),
                    format_f32(max.z)
                ));
                self.state_3d.settings_ui(ui, &self.scene_bbox_accum);
            }
        }
    }

    // TODO(andreas): split into smaller parts, some of it shouldn't be part of the ui path and instead scene loading.
    pub fn view_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        scene: SceneSpatial,
        space_info: &SpaceInfo,
    ) {
        self.scene_bbox_accum = self.scene_bbox_accum.union(scene.primitives.bounding_box());
        self.scene_num_primitives = scene.primitives.num_primitives();

        match self.nav_mode {
            SpatialNavigationMode::ThreeD => {
                let coordinates = space_info.coordinates;
                self.state_3d.space_specs = SpaceSpecs::from_view_coordinates(coordinates);

                super::view_3d(ctx, ui, self, space, scene);
            }
            SpatialNavigationMode::TwoD => {
                let scene_rect_accum = egui::Rect::from_min_max(
                    self.scene_bbox_accum.min.truncate().to_array().into(),
                    self.scene_bbox_accum.max.truncate().to_array().into(),
                );
                super::view_2d(ctx, ui, self, space, scene, scene_rect_accum);
            }
        }
    }

    pub fn help_text(&self) -> &str {
        match self.nav_mode {
            SpatialNavigationMode::TwoD => super::ui_2d::HELP_TEXT,
            SpatialNavigationMode::ThreeD => super::ui_3d::HELP_TEXT,
        }
    }
}

fn size_ui(
    ui: &mut egui::Ui,
    default_size_points: f32,
    default_size_world: f32,
    size: &mut re_renderer::Size,
) {
    use re_renderer::Size;

    let mut mode = if size.is_auto() {
        AutoSizeUnit::Auto
    } else if size.points().is_some() {
        AutoSizeUnit::UiPoints
    } else {
        AutoSizeUnit::World
    };

    let mode_before = mode;
    egui::ComboBox::from_id_source("auto_size_mode")
        .width(80.0)
        .selected_text(mode)
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.selectable_value(&mut mode, AutoSizeUnit::Auto, AutoSizeUnit::Auto)
                .on_hover_text("Determine automatically.");
            ui.selectable_value(&mut mode, AutoSizeUnit::UiPoints, AutoSizeUnit::UiPoints)
                .on_hover_text("Manual in UI points.");
            ui.selectable_value(&mut mode, AutoSizeUnit::World, AutoSizeUnit::World)
                .on_hover_text("Manual in scene units.");
        });
    if mode != mode_before {
        *size = match mode {
            AutoSizeUnit::Auto => Size::AUTO,
            AutoSizeUnit::UiPoints => Size::new_points(default_size_points),
            AutoSizeUnit::World => Size::new_scene(default_size_world),
        };
    }

    if mode != AutoSizeUnit::Auto {
        let mut displayed_size = size.0.abs();
        let (drag_speed, clamp_range) = if mode == AutoSizeUnit::UiPoints {
            (0.1, 0.1..=250.0)
        } else {
            (0.01 * displayed_size, 0.0001..=f32::INFINITY)
        };
        if ui
            .add(
                egui::DragValue::new(&mut displayed_size)
                    .speed(drag_speed)
                    .clamp_range(clamp_range)
                    .max_decimals(4),
            )
            .changed()
        {
            *size = match mode {
                AutoSizeUnit::Auto => unreachable!(),
                AutoSizeUnit::UiPoints => Size::new_points(displayed_size),
                AutoSizeUnit::World => Size::new_scene(displayed_size),
            };
        }
    }
}
