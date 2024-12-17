//! Rerun Time Panel
//!
//! This crate provides a panel that shows allows to control time & timelines,
//! as well as all necessary ui elements that make it up.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod data_density_graph;
mod paint_ticks;
mod recursive_chunks_per_timeline_subscriber;
mod time_axis;
mod time_control_ui;
mod time_ranges_ui;
mod time_selection_ui;

use egui::emath::Rangef;
use egui::{pos2, Color32, CursorIcon, NumExt, Painter, PointerButton, Rect, Shape, Ui, Vec2};

use re_context_menu::{context_menu_ui_for_item, SelectionUpdateBehavior};
use re_data_ui::DataUi as _;
use re_data_ui::{item_ui::guess_instance_path_icon, sorted_component_list_for_ui};
use re_entity_db::{EntityDb, EntityTree, InstancePath};
use re_log_types::{
    external::re_types_core::ComponentName, ComponentPath, EntityPath, EntityPathPart,
    ResolvedTimeRange, TimeInt, TimeReal, TimeType,
};
use re_types::blueprint::components::PanelState;
use re_ui::{list_item, ContextExt as _, DesignTokens, UiExt as _};
use re_viewer_context::{
    CollapseScope, HoverHighlight, Item, RecordingConfig, TimeControl, TimeView, UiLayout,
    ViewerContext,
};
use re_viewport_blueprint::ViewportBlueprint;

use recursive_chunks_per_timeline_subscriber::PathRecursiveChunksPerTimelineStoreSubscriber;
use time_axis::TimelineAxis;
use time_control_ui::TimeControlUi;
use time_ranges_ui::TimeRangesUi;

#[doc(hidden)]
pub mod __bench {
    pub use crate::data_density_graph::*;
    pub use crate::time_ranges_ui::TimeRangesUi;
    pub use crate::TimePanelItem;
}

#[derive(Debug, Clone)]
pub struct TimePanelItem {
    pub entity_path: EntityPath,
    pub component_name: Option<ComponentName>,
}

impl TimePanelItem {
    pub fn entity_path(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            component_name: None,
        }
    }

    pub fn component_path(component_path: ComponentPath) -> Self {
        let ComponentPath {
            entity_path,
            component_name,
        } = component_path;
        Self {
            entity_path,
            component_name: Some(component_name),
        }
    }

    pub fn to_item(&self) -> Item {
        let Self {
            entity_path,
            component_name,
        } = self;

        if let Some(component_name) = component_name {
            Item::ComponentPath(ComponentPath::new(entity_path.clone(), *component_name))
        } else {
            Item::InstancePath(InstancePath::entity_all(entity_path.clone()))
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum TimePanelSource {
    #[default]
    Recording,
    Blueprint,
}

impl From<TimePanelSource> for egui::Id {
    fn from(source: TimePanelSource) -> Self {
        match source {
            TimePanelSource::Recording => "recording".into(),
            TimePanelSource::Blueprint => "blueprint".into(),
        }
    }
}

/// A panel that shows entity names to the left, time on the top.
///
/// This includes the timeline controls and streams view.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TimePanel {
    data_density_graph_painter: data_density_graph::DataDensityGraphPainter,

    /// Width of the entity name columns previous frame.
    prev_col_width: f32,

    /// The right side of the entity name column; updated during its painting.
    #[serde(skip)]
    next_col_right: f32,

    /// The time axis view, regenerated each frame.
    #[serde(skip)]
    time_ranges_ui: TimeRangesUi,

    /// Ui elements for controlling time.
    time_control_ui: TimeControlUi,

    /// Which source is the time panel controlling
    source: TimePanelSource,
}

impl Default for TimePanel {
    fn default() -> Self {
        Self::ensure_registered_subscribers();

        Self {
            data_density_graph_painter: Default::default(),
            prev_col_width: 400.0,
            next_col_right: 0.0,
            time_ranges_ui: Default::default(),
            time_control_ui: TimeControlUi,
            source: TimePanelSource::Recording,
        }
    }
}

impl TimePanel {
    /// Ensures that all required store subscribers are correctly set up.
    ///
    /// This is implicitly called by [`Self::default`], but may need to be explicitly called in,
    /// e.g., testing context.
    pub fn ensure_registered_subscribers() {
        PathRecursiveChunksPerTimelineStoreSubscriber::ensure_registered();
    }

    pub fn new_blueprint_panel() -> Self {
        Self {
            source: TimePanelSource::Blueprint,
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_panel(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        rec_cfg: &RecordingConfig,
        ui: &mut egui::Ui,
        state: PanelState,
        mut panel_frame: egui::Frame,
    ) {
        if state.is_hidden() {
            return;
        }

        self.data_density_graph_painter.begin_frame(ui.ctx());

        // Naturally, many parts of the time panel need the time control.
        // Copy it once, read/edit, and then write back at the end if there was a change.
        let time_ctrl_before = rec_cfg.time_ctrl.read().clone();
        let mut time_ctrl_after = time_ctrl_before.clone();

        // this is the size of everything above the central panel (window title bar, top bar on web,
        // etc.)
        let screen_header_height = ui.cursor().top();

        if state.is_expanded() {
            // Since we use scroll bars we want to fill the whole vertical space downwards:
            panel_frame.inner_margin.bottom = 0.0;

            // Similarly, let the data get close to the right edge:
            panel_frame.inner_margin.right = 0.0;
        }

        let window_height = ui.ctx().screen_rect().height();

        let id: egui::Id = self.source.into();

        let collapsed = egui::TopBottomPanel::bottom(id.with("time_panel_collapsed"))
            .resizable(false)
            .show_separator_line(false)
            .frame(panel_frame)
            .default_height(44.0);

        let min_height = 150.0;
        let min_top_space = 150.0 + screen_header_height;
        let expanded = egui::TopBottomPanel::bottom(id.with("time_panel_expanded"))
            .resizable(true)
            .show_separator_line(false)
            .frame(panel_frame)
            .min_height(min_height)
            .max_height((window_height - min_top_space).at_least(min_height).round())
            .default_height((0.25 * window_height).clamp(min_height, 250.0).round());

        egui::TopBottomPanel::show_animated_between_inside(
            ui,
            state.is_expanded(),
            collapsed,
            expanded,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed or animating
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size =
                            Vec2::splat(re_ui::DesignTokens::top_bar_height());
                        ui.visuals_mut().button_frame = true;
                        self.collapsed_ui(ctx, entity_db, ui, &mut time_ctrl_after);
                    });
                } else {
                    // Expanded:
                    self.show_expanded_with_header(
                        ctx,
                        viewport_blueprint,
                        entity_db,
                        &mut time_ctrl_after,
                        ui,
                    );
                }
            },
        );

        // Apply time control if there were any changes.
        // This means that if anyone else meanwhile changed the time control, these changes are lost now.
        // At least though we don't overwrite them if we didn't change anything at all.
        // Since changes on the time control via the time panel are rare, this should be fine.
        if time_ctrl_before != time_ctrl_after {
            *rec_cfg.time_ctrl.write() = time_ctrl_after;
        }
    }

    pub fn show_expanded_with_header(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &EntityDb,
        time_ctrl_after: &mut TimeControl,
        ui: &mut Ui,
    ) {
        ui.vertical(|ui| {
            // Add back the margin we removed from the panel:
            let mut top_row_frame = egui::Frame::default();
            let margin = DesignTokens::bottom_panel_margin();
            top_row_frame.inner_margin.right = margin.right;
            top_row_frame.inner_margin.bottom = margin.bottom;
            let top_row_rect = top_row_frame
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size =
                            Vec2::splat(re_ui::DesignTokens::top_bar_height());
                        ui.visuals_mut().button_frame = true;
                        self.top_row_ui(ctx, entity_db, ui, time_ctrl_after);
                    });
                })
                .response
                .rect;

            // Draw separator between top bar and the rest:
            ui.painter().hline(
                0.0..=top_row_rect.right(),
                top_row_rect.bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );

            ui.spacing_mut().scroll.bar_outer_margin = 4.0; // needed, because we have no panel margin on the right side.

            // Add extra margin on the left which was intentionally missing on the controls.
            let mut streams_frame = egui::Frame::default();
            streams_frame.inner_margin.left = margin.left;
            streams_frame.show(ui, |ui| {
                self.expanded_ui(ctx, viewport_blueprint, entity_db, ui, time_ctrl_after);
            });
        });
    }

    #[allow(clippy::unused_self)]
    fn collapsed_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        entity_db: &re_entity_db::EntityDb,
        ui: &mut egui::Ui,
        time_ctrl: &mut TimeControl,
    ) {
        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        let time_range = entity_db.time_range_for(time_ctrl.timeline());
        let has_more_than_one_time_point =
            time_range.map_or(false, |time_range| time_range.min() != time_range.max());

        if ui.max_rect().width() < 600.0 && has_more_than_one_time_point {
            // Responsive ui for narrow screens, e.g. mobile. Split the controls into two rows.
            ui.vertical(|ui| {
                if has_more_than_one_time_point {
                    ui.horizontal(|ui| {
                        let times_per_timeline = entity_db.times_per_timeline();
                        self.time_control_ui
                            .play_pause_ui(time_ctrl, times_per_timeline, ui);

                        self.time_control_ui.playback_speed_ui(time_ctrl, ui);
                        self.time_control_ui.fps_ui(time_ctrl, ui);
                    });
                }
                ui.horizontal(|ui| {
                    self.time_control_ui.timeline_selector_ui(
                        time_ctrl,
                        entity_db.times_per_timeline(),
                        ui,
                    );
                    collapsed_time_marker_and_time(
                        ui,
                        ctx,
                        &mut self.data_density_graph_painter,
                        entity_db,
                        time_ctrl,
                    );
                });
            });
        } else {
            // One row:
            let times_per_timeline = entity_db.times_per_timeline();

            if has_more_than_one_time_point {
                self.time_control_ui
                    .play_pause_ui(time_ctrl, times_per_timeline, ui);
            }

            self.time_control_ui
                .timeline_selector_ui(time_ctrl, times_per_timeline, ui);

            if has_more_than_one_time_point {
                self.time_control_ui.playback_speed_ui(time_ctrl, ui);
                self.time_control_ui.fps_ui(time_ctrl, ui);
            }

            collapsed_time_marker_and_time(
                ui,
                ctx,
                &mut self.data_density_graph_painter,
                entity_db,
                time_ctrl,
            );
        }
    }

    fn expanded_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        ui: &mut egui::Ui,
        time_ctrl: &mut TimeControl,
    ) {
        re_tracing::profile_function!();

        //               |timeline            |
        // ------------------------------------
        // tree          |streams             |
        //               |  . .   .   . . .   |
        //               |            . . . . |
        //               ▲
        //               └ tree_max_y (= time_x_left)

        self.next_col_right = ui.min_rect().left(); // next_col_right will expand during the call

        let time_x_left =
            (ui.min_rect().left() + self.prev_col_width + ui.spacing().item_spacing.x)
                .at_most(ui.max_rect().right() - 100.0)
                .at_least(80.); // cover the empty recording case

        // Where the time will be shown.
        let time_bg_x_range = Rangef::new(time_x_left, ui.max_rect().right());
        let time_fg_x_range = {
            // Painting to the right of the scroll bar (if any) looks bad:
            let right = ui.max_rect().right() - ui.spacing_mut().scroll.bar_outer_margin;
            debug_assert!(time_x_left < right);
            Rangef::new(time_x_left, right)
        };

        let side_margin = 26.0; // chosen so that the scroll bar looks approximately centered in the default gap
        self.time_ranges_ui = initialize_time_ranges_ui(
            entity_db,
            time_ctrl,
            Rangef::new(
                time_fg_x_range.min + side_margin,
                time_fg_x_range.max - side_margin,
            ),
            time_ctrl.time_view(),
        );
        let full_y_range = Rangef::new(ui.min_rect().bottom(), ui.max_rect().bottom());

        let timeline_rect = {
            let top = ui.min_rect().bottom();

            let size = egui::vec2(self.prev_col_width, 28.0);
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_min_size(size);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.add_space(4.0); // hack to vertically center the text
                if self.source == TimePanelSource::Blueprint {
                    ui.strong("Blueprint Streams");
                } else {
                    ui.strong("Streams");
                }
            })
            .response
            .on_hover_text(
                "A hierarchical view of the paths used during logging.\n\
                        \n\
                        On the right you can see when there was a log event for a stream.",
            );

            let bottom = ui.min_rect().bottom();
            Rect::from_x_y_ranges(time_fg_x_range, top..=bottom)
        };

        let streams_rect = Rect::from_x_y_ranges(
            time_fg_x_range,
            timeline_rect.bottom()..=ui.max_rect().bottom(),
        );

        // includes the timeline and streams areas.
        let time_bg_area_rect = Rect::from_x_y_ranges(time_bg_x_range, full_y_range);
        let time_fg_area_rect = Rect::from_x_y_ranges(time_fg_x_range, full_y_range);
        let time_bg_area_painter = ui.painter().with_clip_rect(time_bg_area_rect);
        let time_area_painter = ui.painter().with_clip_rect(time_fg_area_rect);

        if let Some(highlighted_range) = time_ctrl.highlighted_range {
            paint_range_highlight(
                highlighted_range,
                &self.time_ranges_ui,
                ui.painter(),
                time_fg_area_rect,
            );
        }

        ui.painter().hline(
            0.0..=ui.max_rect().right(),
            timeline_rect.bottom(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        paint_ticks::paint_time_ranges_and_ticks(
            &self.time_ranges_ui,
            ui,
            &time_area_painter,
            timeline_rect.top()..=timeline_rect.bottom(),
            time_ctrl.time_type(),
            ctx.app_options.time_zone,
        );
        paint_time_ranges_gaps(
            &self.time_ranges_ui,
            ui,
            &time_bg_area_painter,
            full_y_range,
        );
        time_selection_ui::loop_selection_ui(
            time_ctrl,
            &self.time_ranges_ui,
            ui,
            &time_bg_area_painter,
            &timeline_rect,
        );
        let time_area_response = interact_with_streams_rect(
            &self.time_ranges_ui,
            time_ctrl,
            ui,
            &time_bg_area_rect,
            &streams_rect,
        );

        // Don't draw on top of the time ticks
        let lower_time_area_painter = ui.painter().with_clip_rect(Rect::from_x_y_ranges(
            time_fg_x_range,
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        ));

        // All the entity rows and their data density graphs
        ui.full_span_scope(0.0..=time_x_left, |ui| {
            list_item::list_item_scope(ui, "streams_tree", |ui| {
                self.tree_ui(
                    ctx,
                    viewport_blueprint,
                    entity_db,
                    time_ctrl,
                    &time_area_response,
                    &lower_time_area_painter,
                    ui,
                );
            });
        });

        {
            // Paint a shadow between the stream names on the left
            // and the data on the right:
            let shadow_width = 30.0;

            // In the design the shadow starts under the time markers.
            //let shadow_y_start =
            //    timeline_rect.bottom() + ui.visuals().widgets.noninteractive.bg_stroke.width;
            // This looks great but only if there are still time markers.
            // When they move to the right (or have a cut) one expects the shadow to go all the way up.
            // But that's quite complicated so let's have the shadow all the way
            let shadow_y_start = full_y_range.min;

            let shadow_y_end = full_y_range.max;
            let rect = egui::Rect::from_x_y_ranges(
                time_x_left..=(time_x_left + shadow_width),
                shadow_y_start..=shadow_y_end,
            );
            ui.draw_shadow_line(rect, egui::Direction::LeftToRight);
        }

        // Put time-marker on top and last, so that you can always drag it
        time_marker_ui(
            &self.time_ranges_ui,
            time_ctrl,
            ui,
            Some(&time_area_response),
            &time_area_painter,
            &timeline_rect,
        );

        self.time_ranges_ui.snap_time_control(time_ctrl);

        // remember where to show the time for next frame:
        self.prev_col_width = self.next_col_right - ui.min_rect().left();
    }

    // All the entity rows and their data density graphs:
    #[allow(clippy::too_many_arguments)]
    fn tree_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &mut TimeControl,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        ui: &mut egui::Ui,
    ) {
        re_tracing::profile_function!();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            // We turn off `drag_to_scroll` so that the `ScrollArea` don't steal input from
            // the earlier `interact_with_time_area`.
            // We implement drag-to-scroll manually instead!
            .drag_to_scroll(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0; // no spacing needed for ListItems

                if time_area_response.dragged_by(PointerButton::Primary) {
                    ui.scroll_with_delta(Vec2::Y * time_area_response.drag_delta().y);
                }

                // Show "/" on top only for recording streams, because the `/` entity in blueprint
                // is always empty, so it's just lost space. This works around an issue where the
                // selection/hover state of the `/` entity is wrongly synchronized between both
                // stores, due to `Item::*` not tracking stores for entity paths.
                let show_root = self.source == TimePanelSource::Recording;

                if show_root {
                    self.show_tree(
                        ctx,
                        viewport_blueprint,
                        entity_db,
                        time_ctrl,
                        time_area_response,
                        time_area_painter,
                        None,
                        entity_db.tree(),
                        ui,
                        "/",
                    );
                } else {
                    self.show_children(
                        ctx,
                        viewport_blueprint,
                        entity_db,
                        time_ctrl,
                        time_area_response,
                        time_area_painter,
                        entity_db.tree(),
                        ui,
                    );
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn show_tree(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &mut TimeControl,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        last_path_part: Option<&EntityPathPart>,
        tree: &EntityTree,
        ui: &mut egui::Ui,
        show_root_as: &str,
    ) {
        let db = match self.source {
            TimePanelSource::Recording => ctx.recording(),
            TimePanelSource::Blueprint => ctx.store_context.blueprint,
        };

        // The last part of the path component
        let text = if let Some(last_path_part) = last_path_part {
            let stem = last_path_part.ui_string();
            if tree.is_leaf() {
                stem
            } else {
                format!("{stem}/") // show we have children with a /
            }
        } else {
            show_root_as.to_owned()
        };

        let default_open = tree.path.len() <= 1 && !tree.is_leaf();

        let item = TimePanelItem::entity_path(tree.path.clone());
        let is_selected = ctx.selection().contains_item(&item.to_item());
        let is_item_hovered = ctx
            .selection_state()
            .highlight_for_ui_element(&item.to_item())
            == HoverHighlight::Hovered;

        // expand if children is focused
        let focused_entity_path = ctx
            .focused_item
            .as_ref()
            .and_then(|item| item.entity_path());

        if focused_entity_path.is_some_and(|entity_path| entity_path.is_descendant_of(&tree.path)) {
            CollapseScope::StreamsTree
                .entity(tree.path.clone())
                .set_open(ui.ctx(), true);
        }

        // Globally unique id - should only be one of these in view at one time.
        // We do this so that we can support "collapse/expand all" command.
        let id = egui::Id::new(match self.source {
            TimePanelSource::Recording => CollapseScope::StreamsTree.entity(tree.path.clone()),
            TimePanelSource::Blueprint => {
                CollapseScope::BlueprintStreamsTree.entity(tree.path.clone())
            }
        });

        let list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
            ..
        } = ui
            .list_item()
            .selected(is_selected)
            .draggable(true)
            .force_hovered(is_item_hovered)
            .show_hierarchical_with_children(
                ui,
                id,
                default_open,
                list_item::LabelContent::new(text)
                    .with_icon(guess_instance_path_icon(
                        ctx,
                        &InstancePath::from(tree.path.clone()),
                    ))
                    .truncate(false),
                |ui| {
                    self.show_children(
                        ctx,
                        viewport_blueprint,
                        entity_db,
                        time_ctrl,
                        time_area_response,
                        time_area_painter,
                        tree,
                        ui,
                    );
                },
            );

        let response = response.on_hover_ui(|ui| {
            let include_subtree = true;
            re_data_ui::item_ui::entity_hover_card_ui(
                ui,
                ctx,
                &time_ctrl.current_query(),
                db,
                &tree.path,
                include_subtree,
            );
        });

        if Some(&tree.path) == focused_entity_path {
            // Scroll only if the entity isn't already visible. This is important because that's what
            // happens when double-clicking an entity _in the blueprint tree_. In such case, it would be
            // annoying to induce a scroll motion.
            if !ui.clip_rect().contains_rect(response.rect) {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }

        context_menu_ui_for_item(
            ctx,
            viewport_blueprint,
            &item.to_item(),
            &response,
            SelectionUpdateBehavior::UseSelection,
        );
        ctx.handle_select_hover_drag_interactions(&response, item.to_item(), true);

        let is_closed = body_response.is_none();
        let response_rect = response.rect;
        self.next_col_right = self.next_col_right.max(response_rect.right());

        // From the left of the label, all the way to the right-most of the time panel
        let full_width_rect = Rect::from_x_y_ranges(
            response_rect.left()..=ui.max_rect().right(),
            response_rect.y_range(),
        );

        let is_visible = ui.is_rect_visible(full_width_rect);

        // ----------------------------------------------

        // show the data in the time area:
        let tree_has_data_in_current_timeline = entity_db.subtree_has_data_on_timeline(
            &entity_db.storage_engine(),
            time_ctrl.timeline(),
            &tree.path,
        );
        if is_visible && tree_has_data_in_current_timeline {
            let row_rect =
                Rect::from_x_y_ranges(time_area_response.rect.x_range(), response_rect.y_range());

            highlight_timeline_row(ui, ctx, time_area_painter, &item.to_item(), &row_rect);

            // show the density graph only if that item is closed
            if is_closed {
                data_density_graph::data_density_graph_ui(
                    &mut self.data_density_graph_painter,
                    ctx,
                    time_ctrl,
                    db,
                    time_area_painter,
                    ui,
                    &self.time_ranges_ui,
                    row_rect,
                    &item,
                    true,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_children(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &mut TimeControl,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        tree: &EntityTree,
        ui: &mut egui::Ui,
    ) {
        for (last_component, child) in &tree.children {
            self.show_tree(
                ctx,
                viewport_blueprint,
                entity_db,
                time_ctrl,
                time_area_response,
                time_area_painter,
                Some(last_component),
                child,
                ui,
                "/",
            );
        }

        let engine = entity_db.storage_engine();
        let store = engine.store();

        // If this is an entity:
        if let Some(components) = store.all_components_for_entity(&tree.path) {
            for component_name in sorted_component_list_for_ui(components.iter()) {
                let is_static = store.entity_has_static_component(&tree.path, &component_name);

                let component_path = ComponentPath::new(tree.path.clone(), component_name);
                let short_component_name = component_path.component_name.short_name();
                let item = TimePanelItem::component_path(component_path.clone());
                let timeline = time_ctrl.timeline();

                let component_has_data_in_current_timeline = store
                    .entity_has_component_on_timeline(
                        time_ctrl.timeline(),
                        &tree.path,
                        &component_name,
                    );

                let num_static_messages =
                    store.num_static_events_for_component(&tree.path, component_name);
                let num_temporal_messages = store.num_temporal_events_for_component_on_timeline(
                    time_ctrl.timeline(),
                    &tree.path,
                    component_name,
                );
                let total_num_messages = num_static_messages + num_temporal_messages;

                let response = ui
                    .list_item()
                    .selected(ctx.selection().contains_item(&item.to_item()))
                    .force_hovered(
                        ctx.selection_state()
                            .highlight_for_ui_element(&item.to_item())
                            == HoverHighlight::Hovered,
                    )
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new(short_component_name)
                            .with_icon(if is_static {
                                &re_ui::icons::COMPONENT_STATIC
                            } else {
                                &re_ui::icons::COMPONENT_TEMPORAL
                            })
                            .truncate(false),
                    );

                context_menu_ui_for_item(
                    ctx,
                    viewport_blueprint,
                    &item.to_item(),
                    &response,
                    SelectionUpdateBehavior::UseSelection,
                );
                ctx.handle_select_hover_drag_interactions(&response, item.to_item(), false);

                let response_rect = response.rect;

                response.on_hover_ui(|ui| {
                    if total_num_messages == 0 {
                        ui.label(ui.ctx().warning_text(format!(
                            "No event logged on timeline {:?}",
                            timeline.name()
                        )));
                    } else {
                        list_item::list_item_scope(ui, "hover tooltip", |ui| {
                            let kind = if is_static { "Static" } else { "Temporal" };

                            let num_messages = if is_static {
                                num_static_messages
                            } else {
                                num_temporal_messages
                            };

                            let num_messages = if num_messages == 1 {
                                "once".to_owned()
                            } else {
                                format!("{} times", re_format::format_uint(num_messages))
                            };

                            ui.list_item().interactive(false).show_flat(
                                ui,
                                list_item::LabelContent::new(format!(
                                    "{kind} component, logged {num_messages}"
                                ))
                                .truncate(false)
                                .with_icon(if is_static {
                                    &re_ui::icons::COMPONENT_STATIC
                                } else {
                                    &re_ui::icons::COMPONENT_TEMPORAL
                                }),
                            );

                            // Static components are not displayed at all on the timeline, so cannot be
                            // previewed there. So we display their content in this tooltip instead.
                            // Conversely, temporal components change over time, and so showing a specific instance here
                            // can be confusing.
                            if is_static {
                                let query = re_chunk_store::LatestAtQuery::new(
                                    *time_ctrl.timeline(),
                                    TimeInt::MAX,
                                );
                                let ui_layout = UiLayout::Tooltip;
                                component_path.data_ui(ctx, ui, ui_layout, &query, entity_db);
                            }
                        });
                    }
                });

                self.next_col_right = self.next_col_right.max(response_rect.right());

                // From the left of the label, all the way to the right-most of the time panel
                let full_width_rect = Rect::from_x_y_ranges(
                    response_rect.left()..=ui.max_rect().right(),
                    response_rect.y_range(),
                );

                let is_visible = ui.is_rect_visible(full_width_rect);
                if is_visible && component_has_data_in_current_timeline {
                    // show the data in the time area:
                    let row_rect = Rect::from_x_y_ranges(
                        time_area_response.rect.x_range(),
                        response_rect.y_range(),
                    );

                    highlight_timeline_row(ui, ctx, time_area_painter, &item.to_item(), &row_rect);

                    let db = match self.source {
                        TimePanelSource::Recording => ctx.recording(),
                        TimePanelSource::Blueprint => ctx.store_context.blueprint,
                    };

                    data_density_graph::data_density_graph_ui(
                        &mut self.data_density_graph_painter,
                        ctx,
                        time_ctrl,
                        db,
                        time_area_painter,
                        ui,
                        &self.time_ranges_ui,
                        row_rect,
                        &item,
                        true,
                    );
                }
            }
        }
    }

    fn top_row_ui(
        &self,
        ctx: &ViewerContext<'_>,
        entity_db: &re_entity_db::EntityDb,
        ui: &mut egui::Ui,
        time_ctrl: &mut TimeControl,
    ) {
        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        if ui.max_rect().width() < 600.0 {
            // Responsive ui for narrow screens, e.g. mobile. Split the controls into two rows.
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let times_per_timeline = entity_db.times_per_timeline();
                    self.time_control_ui
                        .play_pause_ui(time_ctrl, times_per_timeline, ui);
                    self.time_control_ui.playback_speed_ui(time_ctrl, ui);
                    self.time_control_ui.fps_ui(time_ctrl, ui);
                });
                ui.horizontal(|ui| {
                    self.time_control_ui.timeline_selector_ui(
                        time_ctrl,
                        entity_db.times_per_timeline(),
                        ui,
                    );

                    current_time_ui(ctx, ui, time_ctrl);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        help_button(ui);
                    });
                });
            });
        } else {
            // One row:
            let times_per_timeline = entity_db.times_per_timeline();

            self.time_control_ui
                .play_pause_ui(time_ctrl, times_per_timeline, ui);
            self.time_control_ui
                .timeline_selector_ui(time_ctrl, times_per_timeline, ui);
            self.time_control_ui.playback_speed_ui(time_ctrl, ui);
            self.time_control_ui.fps_ui(time_ctrl, ui);
            current_time_ui(ctx, ui, time_ctrl);
        }
    }
}

/// Draw the hovered/selected highlight background for a timeline row.
fn highlight_timeline_row(
    ui: &Ui,
    ctx: &ViewerContext<'_>,
    painter: &Painter,
    item: &Item,
    row_rect: &Rect,
) {
    let item_hovered =
        ctx.selection_state().highlight_for_ui_element(item) == HoverHighlight::Hovered;
    let item_selected = ctx.selection().contains_item(item);
    let bg_color = if item_selected {
        Some(ui.visuals().selection.bg_fill.gamma_multiply(0.4))
    } else if item_hovered {
        Some(
            ui.visuals()
                .widgets
                .hovered
                .weak_bg_fill
                .gamma_multiply(0.3),
        )
    } else {
        None
    };
    if let Some(bg_color) = bg_color {
        painter.rect_filled(*row_rect, egui::Rounding::ZERO, bg_color);
    }
}

fn collapsed_time_marker_and_time(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    data_density_graph_painter: &mut data_density_graph::DataDensityGraphPainter,
    entity_db: &re_entity_db::EntityDb,
    time_ctrl: &mut TimeControl,
) {
    let timeline = time_ctrl.timeline();

    let Some(time_range) = entity_db.time_range_for(timeline) else {
        // We have no data on this timeline
        return;
    };

    if time_range.min() == time_range.max() {
        // Only one time point - showing a slider that can't be moved is just annoying
    } else {
        let space_needed_for_current_time = match timeline.typ() {
            re_chunk_store::TimeType::Time => 220.0,
            re_chunk_store::TimeType::Sequence => 100.0,
        };

        let mut time_range_rect = ui.available_rect_before_wrap();
        time_range_rect.max.x -= space_needed_for_current_time;

        if time_range_rect.width() > 50.0 {
            ui.allocate_rect(time_range_rect, egui::Sense::hover());

            let time_ranges_ui =
                initialize_time_ranges_ui(entity_db, time_ctrl, time_range_rect.x_range(), None);
            time_ranges_ui.snap_time_control(time_ctrl);

            let painter = ui.painter_at(time_range_rect.expand(4.0));

            if let Some(highlighted_range) = time_ctrl.highlighted_range {
                paint_range_highlight(
                    highlighted_range,
                    &time_ranges_ui,
                    &painter,
                    time_range_rect,
                );
            }

            painter.hline(
                time_range_rect.x_range(),
                time_range_rect.center().y,
                ui.visuals().widgets.noninteractive.fg_stroke,
            );

            data_density_graph::data_density_graph_ui(
                data_density_graph_painter,
                ctx,
                time_ctrl,
                entity_db,
                ui.painter(),
                ui,
                &time_ranges_ui,
                time_range_rect.shrink2(egui::vec2(0.0, 10.0)),
                &TimePanelItem::entity_path(EntityPath::root()),
                false,
            );

            time_marker_ui(
                &time_ranges_ui,
                time_ctrl,
                ui,
                None,
                &painter,
                &time_range_rect,
            );
        }
    }

    current_time_ui(ctx, ui, time_ctrl);
}

fn paint_range_highlight(
    highlighted_range: ResolvedTimeRange,
    time_ranges_ui: &TimeRangesUi,
    painter: &egui::Painter,
    rect: Rect,
) {
    let x_from = time_ranges_ui.x_from_time_f32(highlighted_range.min().into());
    let x_to = time_ranges_ui.x_from_time_f32(highlighted_range.max().into());

    if let (Some(x_from), Some(x_to)) = (x_from, x_to) {
        let visible_history_area_rect =
            Rect::from_x_y_ranges(x_from..=x_to, rect.y_range()).intersect(rect);

        painter.rect(
            visible_history_area_rect,
            0.0,
            egui::Color32::WHITE.gamma_multiply(0.1),
            egui::Stroke::NONE,
        );
    }
}

fn help_button(ui: &mut egui::Ui) {
    // TODO(andreas): Nicer help text like on views.
    ui.help_hover_button().on_hover_text(
        "\
        In the top row you can drag to move the time, or shift-drag to select a loop region.\n\
        \n\
        Drag main area to pan.\n\
        Zoom: Ctrl/cmd + scroll, or drag up/down with secondary mouse button.\n\
        Double-click to reset view.\n\
        \n\
        Press the space bar to play/pause.",
    );
}

fn current_time_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, time_ctrl: &mut TimeControl) {
    if let Some(time_int) = time_ctrl.time_int() {
        let time_type = time_ctrl.time_type();
        match time_type {
            re_log_types::TimeType::Time => {
                // TODO(#7653): parse time stamps
                ui.monospace(time_type.format(time_int, ctx.app_options.time_zone));
            }
            re_log_types::TimeType::Sequence => {
                // NOTE: egui uses `f64` for all numbers internally, so we get precision problems if the integer gets too big.
                if time_int.as_f64() as i64 == time_int.as_i64() {
                    let mut int = time_int.as_i64();
                    let drag_value = egui::DragValue::new(&mut int)
                        .custom_formatter(|x, _range| {
                            TimeType::format_sequence(TimeInt::new_temporal(x as i64))
                        })
                        .custom_parser(|s| TimeType::parse_sequence(s).map(TimeInt::as_f64));
                    let response = ui.add(drag_value);
                    if response.changed() {
                        time_ctrl.set_time(TimeInt::new_temporal(int));
                    }
                } else {
                    // Avoid the precision problems by just displaying the number without the ability to change it (here).
                    ui.monospace(time_type.format(time_int, ctx.app_options.time_zone));
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------

fn initialize_time_ranges_ui(
    entity_db: &re_entity_db::EntityDb,
    time_ctrl: &TimeControl,
    time_x_range: Rangef,
    mut time_view: Option<TimeView>,
) -> TimeRangesUi {
    re_tracing::profile_function!();

    let mut time_range = Vec::new();

    if let Some(times) = entity_db.time_histogram(time_ctrl.timeline()) {
        // NOTE: `times` can be empty if a GC wiped everything.
        if !times.is_empty() {
            let timeline_axis = TimelineAxis::new(time_ctrl.time_type(), times);
            time_view = time_view.or_else(|| Some(view_everything(&time_x_range, &timeline_axis)));
            time_range.extend(timeline_axis.ranges);
        }
    }

    TimeRangesUi::new(
        time_x_range,
        time_view.unwrap_or(TimeView {
            min: TimeReal::from(0),
            time_spanned: 1.0,
        }),
        &time_range,
    )
}

/// Find a nice view of everything.
fn view_everything(x_range: &Rangef, timeline_axis: &TimelineAxis) -> TimeView {
    let gap_width = time_ranges_ui::gap_width(x_range, &timeline_axis.ranges) as f32;
    let num_gaps = timeline_axis.ranges.len().saturating_sub(1);
    let width = x_range.span();
    let width_sans_gaps = width - num_gaps as f32 * gap_width;

    let factor = if width_sans_gaps > 0.0 {
        width / width_sans_gaps
    } else {
        1.0 // too narrow to fit everything anyway
    };

    let min = timeline_axis.min();
    let time_spanned = timeline_axis.sum_time_lengths() as f64 * factor as f64;

    TimeView {
        min: min.into(),
        time_spanned,
    }
}

/// Visually separate the different time segments
fn paint_time_ranges_gaps(
    time_ranges_ui: &TimeRangesUi,
    ui: &egui::Ui,
    painter: &egui::Painter,
    y_range: Rangef,
) {
    re_tracing::profile_function!();

    // For each gap we are painting this:
    //
    //             zig width
    //             |
    //            <->
    //    \         /  ^
    //     \       /   | zig height
    //      \     /    v
    //      /     \
    //     /       \
    //    /         \
    //    \         /
    //     \       /
    //      \     /
    //      /     \
    //     /       \
    //    /         \
    //
    //    <--------->
    //     gap width
    //
    // Filled with a dark color, plus a stroke and a small drop shadow to the left.

    use itertools::Itertools as _;

    let Rangef {
        min: top,
        max: bottom,
    } = y_range;

    let fill_color = ui.visuals().widgets.noninteractive.bg_fill;
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;

    let paint_time_gap = |gap_left: f32, gap_right: f32| {
        let gap_width = gap_right - gap_left;
        let zig_width = 4.0_f32.at_most(gap_width / 3.0).at_least(1.0);
        let zig_height = zig_width;
        let shadow_width = 12.0;

        let mut y = top;
        let mut row = 0; // 0 = start wide, 1 = start narrow

        let mut mesh = egui::Mesh::default();
        let mut shadow_mesh = egui::Mesh::default();
        let mut left_line_strip = vec![];
        let mut right_line_strip = vec![];

        while y - zig_height <= bottom {
            let (left, right) = if row % 2 == 0 {
                // full width
                (gap_left, gap_right)
            } else {
                // contracted
                (gap_left + zig_width, gap_right - zig_width)
            };

            let left_pos = pos2(left, y);
            let right_pos = pos2(right, y);

            if !mesh.is_empty() {
                let next_left_vidx = mesh.vertices.len() as u32;
                let next_right_vidx = next_left_vidx + 1;
                let prev_left_vidx = next_left_vidx - 2;
                let prev_right_vidx = next_right_vidx - 2;

                mesh.add_triangle(prev_left_vidx, next_left_vidx, prev_right_vidx);
                mesh.add_triangle(next_left_vidx, prev_right_vidx, next_right_vidx);
            }

            mesh.colored_vertex(left_pos, fill_color);
            mesh.colored_vertex(right_pos, fill_color);

            shadow_mesh.colored_vertex(pos2(right - shadow_width, y), Color32::TRANSPARENT);
            shadow_mesh
                .colored_vertex(right_pos, re_ui::design_tokens().shadow_gradient_dark_start);

            left_line_strip.push(left_pos);
            right_line_strip.push(right_pos);

            y += zig_height;
            row += 1;
        }

        // Regular & shadow mesh have the same topology!
        shadow_mesh.indices.clone_from(&mesh.indices);

        painter.add(Shape::Mesh(mesh));
        painter.add(Shape::Mesh(shadow_mesh));
        painter.add(Shape::line(left_line_strip, stroke));
        painter.add(Shape::line(right_line_strip, stroke));
    };

    let zig_zag_first_and_last_edges = true;

    if let Some(segment) = time_ranges_ui.segments.first() {
        let gap_edge = *segment.x.start() as f32;

        if zig_zag_first_and_last_edges {
            // Left side of first segment - paint as a very wide gap that we only see the right side of
            paint_time_gap(gap_edge - 100_000.0, gap_edge);
        } else {
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge - 100_000.0, top), pos2(gap_edge, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range, stroke);
        }
    }

    for (a, b) in time_ranges_ui.segments.iter().tuple_windows() {
        paint_time_gap(*a.x.end() as f32, *b.x.start() as f32);
    }

    if let Some(segment) = time_ranges_ui.segments.last() {
        let gap_edge = *segment.x.end() as f32;
        if zig_zag_first_and_last_edges {
            // Right side of last segment - paint as a very wide gap that we only see the left side of
            paint_time_gap(gap_edge, gap_edge + 100_000.0);
        } else {
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge, top), pos2(gap_edge + 100_000.0, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range, stroke);
        }
    }
}

/// Returns a scroll delta
#[must_use]
fn interact_with_streams_rect(
    time_ranges_ui: &TimeRangesUi,
    time_ctrl: &mut TimeControl,
    ui: &egui::Ui,
    full_rect: &Rect,
    streams_rect: &Rect,
) -> egui::Response {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    let mut delta_x = 0.0;
    let mut zoom_factor = 1.0;

    // Check for zoom/pan inputs (via e.g. horizontal scrolling) on the entire
    // time area rectangle, including the timeline rect.
    let full_rect_hovered =
        pointer_pos.map_or(false, |pointer_pos| full_rect.contains(pointer_pos));
    if full_rect_hovered {
        ui.input(|input| {
            delta_x += input.smooth_scroll_delta.x;
            zoom_factor *= input.zoom_delta_2d().x;
        });
    }

    // We only check for drags in the streams rect,
    // because drags in the timeline rect should move the time
    // (or create loop sections).
    let response = ui.interact(
        *streams_rect,
        ui.id().with("time_area_interact"),
        egui::Sense::click_and_drag(),
    );
    if response.dragged_by(PointerButton::Primary) {
        delta_x += response.drag_delta().x;
        ui.ctx().set_cursor_icon(CursorIcon::AllScroll);
    }
    if response.dragged_by(PointerButton::Secondary) {
        zoom_factor *= (response.drag_delta().y * 0.01).exp();
    }

    if delta_x != 0.0 {
        if let Some(new_view_range) = time_ranges_ui.pan(-delta_x) {
            time_ctrl.set_time_view(new_view_range);
        }
    }

    if zoom_factor != 1.0 {
        if let Some(pointer_pos) = pointer_pos {
            if let Some(new_view_range) = time_ranges_ui.zoom_at(pointer_pos.x, zoom_factor) {
                time_ctrl.set_time_view(new_view_range);
            }
        }
    }

    if response.double_clicked() {
        time_ctrl.reset_time_view();
    }

    response
}

/// A vertical line that shows the current time.
fn time_marker_ui(
    time_ranges_ui: &TimeRangesUi,
    time_ctrl: &mut TimeControl,
    ui: &egui::Ui,
    time_area_response: Option<&egui::Response>,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
) {
    // timeline_rect: top part with the second ticks and time marker

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let time_drag_id = ui.id().with("time_drag_id");
    let timeline_cursor_icon = CursorIcon::ResizeHorizontal;
    let is_hovering_the_loop_selection = ui.output(|o| o.cursor_icon) != CursorIcon::Default; // A kind of hacky proxy
    let is_anything_being_dragged = ui.ctx().dragged_id().is_some();
    let time_area_double_clicked = time_area_response.is_some_and(|resp| resp.double_clicked());
    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    let mut is_hovering_time_cursor = false;

    // show current time as a line:
    if let Some(time) = time_ctrl.time() {
        if let Some(mut x) = time_ranges_ui.x_from_time_f32(time) {
            if timeline_rect.x_range().contains(x) {
                let line_rect =
                    Rect::from_x_y_ranges(x..=x, timeline_rect.top()..=ui.max_rect().bottom())
                        .expand(interact_radius);

                let sense = if time_area_double_clicked {
                    egui::Sense::hover()
                } else {
                    egui::Sense::drag()
                };

                let response = ui
                    .interact(line_rect, time_drag_id, sense)
                    .on_hover_and_drag_cursor(timeline_cursor_icon);

                is_hovering_time_cursor = response.hovered();

                if response.dragged() {
                    if let Some(pointer_pos) = pointer_pos {
                        if let Some(time) = time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                            let time = time_ranges_ui.clamp_time(time);
                            time_ctrl.set_time(time);
                            time_ctrl.pause();

                            x = pointer_pos.x; // avoid frame-delay
                        }
                    }
                }

                ui.paint_time_cursor(
                    time_area_painter,
                    &response,
                    x,
                    Rangef::new(timeline_rect.top(), ui.max_rect().bottom()),
                );
            }
        }
    }

    // "click here to view time here"
    if let Some(pointer_pos) = pointer_pos {
        let is_pointer_in_time_area_rect =
            ui.ui_contains_pointer() && time_area_painter.clip_rect().contains(pointer_pos);
        let is_pointer_in_timeline_rect =
            ui.ui_contains_pointer() && timeline_rect.contains(pointer_pos);

        // Show preview?
        if !is_hovering_time_cursor
            && !time_area_double_clicked
            && is_pointer_in_time_area_rect
            && !is_anything_being_dragged
            && !is_hovering_the_loop_selection
        {
            time_area_painter.vline(
                pointer_pos.x,
                timeline_rect.top()..=ui.max_rect().bottom(),
                ui.visuals().widgets.noninteractive.fg_stroke,
            );
            ui.ctx().set_cursor_icon(timeline_cursor_icon); // preview!
        }

        // Click to move time here:
        let time_area_response = ui.interact(
            time_area_painter.clip_rect(),
            ui.id().with("time_area_painter_id"),
            egui::Sense::click(),
        );

        if !is_hovering_the_loop_selection {
            let mut set_time_to_pointer = || {
                if let Some(time) = time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                    let time = time_ranges_ui.clamp_time(time);
                    time_ctrl.set_time(time);
                    time_ctrl.pause();
                }
            };

            // click on timeline = set time + start drag
            // click on time area = set time
            // double click on time area = reset time
            if !is_anything_being_dragged
                && is_pointer_in_timeline_rect
                && ui.input(|i| i.pointer.primary_down())
            {
                set_time_to_pointer();
                ui.ctx().set_dragged_id(time_drag_id);
            } else if is_pointer_in_time_area_rect {
                if time_area_response.double_clicked() {
                    time_ctrl.reset_time_view();
                } else if time_area_response.clicked() && !is_anything_being_dragged {
                    set_time_to_pointer();
                }
            }
        }
    }
}
