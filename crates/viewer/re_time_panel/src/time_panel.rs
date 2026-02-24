use std::sync::Arc;

use egui::emath::Rangef;
use egui::scroll_area::ScrollSource;
use egui::{
    Color32, CursorIcon, Modifiers, NumExt as _, Painter, PointerButton, Rect, Response, RichText,
    Shape, Ui, Vec2, WidgetInfo, WidgetType, pos2,
};
use re_context_menu::{SelectionUpdateBehavior, context_menu_ui_for_item_with_context};
use re_data_ui::DataUi as _;
use re_data_ui::item_ui::guess_instance_path_icon;
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{
    AbsoluteTimeRange, ApplicationId, ComponentPath, EntityPath, TimeInt, TimeReal,
};
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::components::PanelState;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_ui::filter_widget::format_matching_text;
use re_ui::{
    ContextExt as _, DesignTokens, Help, IconText, UiExt as _, filter_widget, icons, list_item,
};
use re_viewer_context::open_url::ViewerOpenUrl;
use re_viewer_context::{
    CollapseScope, HoverHighlight, Item, ItemCollection, ItemContext, SystemCommand,
    SystemCommandSender as _, TimeControl, TimeControlCommand, TimeView, UiLayout, ViewerContext,
    VisitorControlFlow,
};
use re_viewport_blueprint::ViewportBlueprint;

use crate::recursive_chunks_per_timeline_subscriber::PathRecursiveChunksPerTimelineStoreSubscriber;
use crate::streams_tree_data::{EntityData, StreamsTreeData, components_for_entity};
use crate::time_axis::TimelineAxis;
use crate::time_control_ui::TimeControlUi;
use crate::time_ranges_ui::{self, TimeRangesUi};
use crate::{MOVE_TIME_CURSOR_ICON, data_density_graph, paint_ticks, time_selection_ui};

#[derive(Debug, Clone)]
pub struct TimePanelItem {
    pub entity_path: EntityPath,
    pub component: Option<ComponentIdentifier>,
}

impl TimePanelItem {
    pub fn entity_path(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            component: None,
        }
    }

    pub fn to_item(&self) -> Item {
        let Self {
            entity_path,
            component,
        } = self;

        if let Some(component) = *component {
            Item::ComponentPath(ComponentPath::new(entity_path.clone(), component))
        } else {
            Item::InstancePath(InstancePath::entity_all(entity_path.clone()))
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum TimePanelSource {
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

impl From<TimePanelSource> for re_log_types::StoreKind {
    fn from(source: TimePanelSource) -> Self {
        match source {
            TimePanelSource::Recording => Self::Recording,
            TimePanelSource::Blueprint => Self::Blueprint,
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

    /// Which source is the time panel controlling?
    source: TimePanelSource,

    /// Filtering of entity paths shown in the panel (when expanded).
    #[serde(skip)]
    filter_state: filter_widget::FilterState,

    /// The store id the filter widget relates to.
    ///
    /// Used to invalidate the filter state (aka deactivate it) when the user switches to a
    /// recording with a different application id.
    #[serde(skip)]
    filter_state_app_id: Option<ApplicationId>,

    /// Range selection anchor item.
    ///
    /// This is the item we used as a starting point for range selection. It is set and remembered
    /// everytime the user clicks on an item _without_ holding shift.
    #[serde(skip)]
    range_selection_anchor_item: Option<Item>,

    /// Used when the selection is modified using key navigation.
    ///
    /// IMPORTANT: Always make sure that the item will be drawn this or next frame when setting this
    /// to `Some`, so that this flag is immediately consumed.
    #[serde(skip)]
    scroll_to_me_item: Option<Item>,

    /// If the timestamp is being edited, the current value.
    ///
    /// It is applied only after removing focus.
    #[serde(skip)]
    pub time_edit_string: Option<String>,

    /// If we're hovering a specific event - what time is it?
    #[serde(skip)]
    hovered_event_time: Option<TimeInt>,
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
            filter_state: Default::default(),
            filter_state_app_id: None,
            range_selection_anchor_item: None,
            scroll_to_me_item: None,
            time_edit_string: None,
            hovered_event_time: None,
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

    /// Activates the search filter (for e.g. test purposes).
    pub fn activate_filter(&mut self, query: &str) {
        self.filter_state.activate(query);
    }

    #[expect(clippy::too_many_arguments)]
    pub fn show_panel(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        state: PanelState,
        mut panel_frame: egui::Frame,
    ) {
        if state.is_hidden() {
            return;
        }

        let tokens = ui.tokens();

        // Invalidate the filter widget if the store id has changed.
        if self.filter_state_app_id.as_ref() != Some(ctx.store_context.application_id()) {
            self.filter_state = Default::default();
            self.filter_state_app_id = Some(ctx.store_context.application_id().clone());
        }

        self.data_density_graph_painter.begin_frame(ui.ctx());
        self.hovered_event_time = None;

        let mut time_commands = Vec::new();

        // this is the size of everything above the central panel (window title bar, top bar on web,
        // etc.)
        let screen_header_height = ui.cursor().top();

        if state.is_expanded() {
            // Since we use scroll bars we want to fill the whole vertical space downwards:
            panel_frame.inner_margin.bottom = 0;

            // Similarly, let the data get close to the right edge:
            panel_frame.inner_margin.right = 0;
        }

        let window_height = ui.ctx().content_rect().height();

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
                        ui.spacing_mut().interact_size = Vec2::splat(tokens.top_bar_height());
                        ui.visuals_mut().button_frame = true;
                        self.collapsed_ui(entity_db, ctx, time_ctrl, ui, &mut time_commands);
                    });
                } else {
                    // Expanded:
                    self.show_expanded_with_header(
                        ctx,
                        time_ctrl,
                        viewport_blueprint,
                        entity_db,
                        ui,
                        &mut time_commands,
                    );
                }
            },
        );

        if !time_commands.is_empty() {
            ctx.command_sender()
                .send_system(SystemCommand::TimeControlCommands {
                    store_id: entity_db.store_id().clone(),
                    time_commands,
                });
        }
    }

    pub fn show_expanded_with_header(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &EntityDb,
        ui: &mut Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let tokens = ui.tokens();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.5;
            // Add back the margin we removed from the panel:
            let mut top_row_frame = egui::Frame::default();
            let margin = tokens.bottom_panel_margin();
            top_row_frame.inner_margin.right = margin.right;
            top_row_frame.inner_margin.bottom = margin.bottom;
            let top_row_rect = top_row_frame
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size = Vec2::splat(tokens.top_bar_height());
                        ui.visuals_mut().button_frame = true;
                        self.top_row_ui(ctx, time_ctrl, entity_db, ui, time_commands);
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
                self.expanded_ui(
                    ctx,
                    time_ctrl,
                    viewport_blueprint,
                    entity_db,
                    ui,
                    time_commands,
                    top_row_rect.bottom(),
                );
            });
        });
    }

    fn collapsed_ui(
        &mut self,
        entity_db: &re_entity_db::EntityDb,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        re_tracing::profile_function!();

        let loading_text = if entity_db.is_currently_downloading_manifest() {
            Some("Downloading meta-data")
        } else if time_ctrl.is_pending() {
            Some("Waiting for timeline")
        } else {
            None
        };

        if let Some(loading_text) = loading_text {
            ui.horizontal(|ui| {
                ui.centered_and_justified(|ui| {
                    ui.loading_indicator(loading_text)
                        .on_hover_text(loading_text);
                });
            });
            return;
        }

        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        let time_range = entity_db.time_range_for(time_ctrl.timeline_name());
        let has_more_than_one_time_point =
            time_range.is_some_and(|time_range| time_range.min() != time_range.max());

        if ui.max_rect().width() < 600.0 && has_more_than_one_time_point {
            // Responsive ui for narrow screens, e.g. mobile. Split the controls into two rows.
            ui.vertical(|ui| {
                if has_more_than_one_time_point {
                    ui.horizontal(|ui| {
                        self.time_control_ui
                            .play_pause_ui(time_ctrl, ui, time_commands);

                        self.time_control_ui
                            .playback_speed_ui(time_ctrl, ui, time_commands);
                        self.time_control_ui.fps_ui(time_ctrl, ui, time_commands);
                    });
                }
                ui.horizontal(|ui| {
                    self.time_control_ui.timeline_selector_ui(
                        time_ctrl,
                        entity_db.timeline_histograms(),
                        ui,
                        time_commands,
                    );
                    self.collapsed_time_marker_and_time(
                        ui,
                        ctx,
                        time_ctrl,
                        entity_db,
                        time_commands,
                    );
                });
            });
        } else {
            // One row:
            let timeline_histograms = entity_db.timeline_histograms();

            if has_more_than_one_time_point {
                self.time_control_ui
                    .play_pause_ui(time_ctrl, ui, time_commands);
            }

            self.time_control_ui.timeline_selector_ui(
                time_ctrl,
                timeline_histograms,
                ui,
                time_commands,
            );

            if has_more_than_one_time_point {
                self.time_control_ui
                    .playback_speed_ui(time_ctrl, ui, time_commands);
                self.time_control_ui.fps_ui(time_ctrl, ui, time_commands);
            }

            self.collapsed_time_marker_and_time(ui, ctx, time_ctrl, entity_db, time_commands);
        }
    }

    #[expect(clippy::too_many_arguments)]
    fn expanded_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
        top_row_y: f32,
    ) {
        re_tracing::profile_function!();

        if entity_db.is_currently_downloading_manifest() {
            ui.loading_screen_ui("Downloading meta-data", |ui| {
                let text = "Downloading meta-data";
                ui.label(egui::RichText::from(text).heading().strong());
            });

            return;
        }

        if time_ctrl.is_pending() {
            ui.loading_screen_ui("Waiting for timeline", |ui| {
                let text = format!("Waiting for timeline: {}", time_ctrl.timeline_name());
                ui.label(egui::RichText::from(text).heading().strong());

                let timeline_count = entity_db.timelines().len();

                match timeline_count {
                    0 => {}
                    1 => {
                        ui.label("One other timeline has data");
                    }
                    c => {
                        ui.label(format!("{c} other timelines have data"));
                    }
                }

                if ui
                    .button(
                        egui::RichText::new("Go to default timeline")
                            .color(ui.style().visuals.weak_text_color()),
                    )
                    .clicked()
                {
                    time_commands.push(TimeControlCommand::ResetActiveTimeline);
                }
            });

            return;
        }

        //               |timeline            |
        // ------------------------------------
        // tree          |streams             |
        //               |  . .   .   . . .   |
        //               |            . . . . |
        //               ▲
        //               └ tree_max_y (= time_x_left)

        // We use this to track what the rightmost coordinate for the tree section should be. We
        // clamp it to a minimum of 150.0px for the filter widget to behave correctly even when the
        // tree is fully collapsed (and thus narrow).
        self.next_col_right = ui.min_rect().left() + 150.0;

        let time_x_left =
            (ui.min_rect().left() + self.prev_col_width + ui.spacing().item_spacing.x)
                .at_most(ui.max_rect().right() - 100.0)
                .at_least(80.); // cover the empty recording case

        // Where the time will be shown.
        let time_bg_x_range = Rangef::new(time_x_left, ui.max_rect().right());
        let time_fg_x_range = {
            // Painting to the right of the scroll bar (if any) looks bad:
            let right = ui.max_rect().right() - ui.spacing_mut().scroll.bar_outer_margin;
            re_log::debug_assert!(time_x_left < right);
            Rangef::new(time_x_left, right)
        };

        let draw_loaded_ranges = true;
        data_density_graph::paint_loaded_indicator_bar(
            ui,
            &self.time_ranges_ui,
            entity_db,
            time_ctrl,
            top_row_y,
            time_fg_x_range,
            draw_loaded_ranges,
        );

        let side_margin = 26.0; // chosen so that the scroll bar looks approximately centered in the default gap
        self.time_ranges_ui = initialize_time_ranges_ui(
            time_ctrl,
            entity_db,
            Rangef::new(
                time_fg_x_range.min + side_margin,
                time_fg_x_range.max - side_margin,
            ),
            time_ctrl.time_view(),
        );
        let full_y_range = Rangef::new(ui.max_rect().top(), ui.max_rect().bottom());

        let timeline_rect = {
            let top = ui.min_rect().bottom();
            ui.response()
                .widget_info(|| WidgetInfo::labeled(WidgetType::Panel, true, "_streams_tree"));

            let size = egui::vec2(self.prev_col_width, DesignTokens::list_item_height());
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_min_size(size);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.spacing_mut().item_spacing.y = 0.0;

                ui.full_span_scope(0.0..=time_x_left, |ui| {
                    self.filter_state.section_title_ui(
                        ui,
                        egui::RichText::new(if self.source == TimePanelSource::Blueprint {
                            "Blueprint Streams"
                        } else {
                            "Streams"
                        })
                        .strong(),
                    );
                });
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
            timeline_rect.left()..=ui.max_rect().right(),
            timeline_rect.bottom(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        if let Some(time_type) = time_ctrl.time_type() {
            paint_ticks::paint_time_ranges_and_ticks(
                &self.time_ranges_ui,
                ui,
                &time_area_painter,
                timeline_rect.y_range(),
                time_type,
                ctx.app_options().timestamp_format,
            );
        }
        paint_time_ranges_gaps(
            &self.time_ranges_ui,
            ui,
            &time_bg_area_painter,
            full_y_range,
        );
        time_selection_ui::loop_selection_ui(
            ctx,
            time_ctrl,
            &self.time_ranges_ui,
            ui,
            &time_bg_area_painter,
            &timeline_rect,
            time_commands,
        );
        let time_area_response = pan_and_zoom_interaction(
            &self.time_ranges_ui,
            ui,
            &time_bg_area_rect,
            &streams_rect,
            time_commands,
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
                    time_ctrl,
                    viewport_blueprint,
                    entity_db,
                    &time_area_response,
                    &lower_time_area_painter,
                    ui,
                    time_commands,
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
        self.time_marker_ui(
            ui,
            ctx,
            time_ctrl,
            Some(time_area_response),
            &time_area_painter,
            &timeline_rect,
            &streams_rect,
            time_commands,
        );

        self.time_ranges_ui
            .snap_time_control(time_ctrl, time_commands);

        // remember where to show the time for next frame:
        self.prev_col_width = self.next_col_right - ui.min_rect().left();
    }

    // All the entity rows and their data density graphs:
    #[expect(clippy::too_many_arguments)]
    fn tree_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        viewport_blueprint: &ViewportBlueprint,
        entity_db: &re_entity_db::EntityDb,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        re_tracing::profile_function!();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            // We turn off `ScrollSource::DRAG` so that the `ScrollArea` don't steal input from
            // the earlier `pan_and_zoom_interaction`.
            // We implement drag-to-scroll manually instead, with middle mouse button
            .scroll_source(ScrollSource::MOUSE_WHEEL | ScrollSource::SCROLL_BAR)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0; // no spacing needed for ListItems

                if time_area_response.dragged_by(PointerButton::Middle) {
                    ui.scroll_with_delta(Vec2::Y * time_area_response.drag_delta().y);
                }

                let filter_matcher = self.filter_state.filter();

                let streams_tree_data =
                    crate::streams_tree_data::StreamsTreeData::from_source_and_filter(
                        ctx,
                        self.source,
                        &filter_matcher,
                    );

                for child in &streams_tree_data.children {
                    self.show_entity(
                        ctx,
                        time_ctrl,
                        viewport_blueprint,
                        &streams_tree_data,
                        entity_db,
                        time_area_response,
                        time_area_painter,
                        child,
                        ui,
                        time_commands,
                    );
                }
            });
    }

    /// Display the list item for an entity.
    #[expect(clippy::too_many_arguments)]
    fn show_entity(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        viewport_blueprint: &ViewportBlueprint,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        entity_data: &EntityData,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        re_tracing::profile_function!();

        let entity_path = &entity_data.entity_path;

        let item = TimePanelItem::entity_path(entity_path.clone());
        let is_selected = ctx.selection().contains_item(&item.to_item());
        let is_item_hovered = ctx
            .selection_state()
            .highlight_for_ui_element(&item.to_item())
            == HoverHighlight::Hovered;

        let collapse_scope = self.collapse_scope();

        // Expand if one of the children is focused
        let focused_entity_path = ctx
            .focused_item()
            .as_ref()
            .and_then(|item| item.entity_path());

        if focused_entity_path
            .is_some_and(|focused_entity_path| focused_entity_path.is_descendant_of(entity_path))
        {
            collapse_scope
                .entity(entity_path.clone())
                .set_open(ui.ctx(), true);
        }

        // Globally unique id that is dependent on the "nature" of the tree (recording or blueprint,
        // in a filter session or not)
        let id = collapse_scope.entity(entity_path.clone()).into();

        let list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
            ..
        } = ui
            .list_item()
            .render_offscreen(false)
            .selected(is_selected)
            .draggable(true)
            .force_hovered(is_item_hovered)
            .show_hierarchical_with_children(
                ui,
                id,
                entity_data.default_open,
                list_item::LabelContent::new(format_matching_text(
                    ctx.egui_ctx(),
                    &entity_data.label,
                    entity_data.highlight_sections.iter().cloned(),
                    None,
                ))
                .with_icon(guess_instance_path_icon(
                    ctx,
                    &InstancePath::from(entity_path.clone()),
                ))
                .truncate(false),
                |ui| {
                    self.show_entity_contents(
                        ctx,
                        time_ctrl,
                        viewport_blueprint,
                        streams_tree_data,
                        entity_db,
                        time_area_response,
                        time_area_painter,
                        entity_data,
                        ui,
                        time_commands,
                    );
                },
            );

        let response = response.on_hover_ui(|ui| {
            let include_subtree = true;
            re_data_ui::item_ui::entity_hover_card_ui(
                ui,
                ctx,
                &time_ctrl.current_query(),
                entity_db,
                entity_path,
                include_subtree,
            );
        });

        if Some(entity_path) == focused_entity_path {
            // Scroll only if the entity isn't already visible. This is important because that's what
            // happens when double-clicking an entity _in the blueprint tree_. In such case, it would be
            // annoying to induce a scroll motion.
            if !ui.clip_rect().contains_rect(response.rect) {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }

        self.handle_interactions_for_item(
            ctx,
            viewport_blueprint,
            streams_tree_data,
            entity_db,
            item.to_item(),
            &response,
            true,
        );

        let is_closed = body_response.is_none();
        let response_rect = response.rect;
        self.next_col_right = self.next_col_right.max(response_rect.right());

        //
        // Display the data density graph only if it is visible.
        //

        // From the left of the label, all the way to the right-most of the time panel
        let full_width_rect = Rect::from_x_y_ranges(
            response_rect.left()..=ui.max_rect().right(),
            response_rect.y_range(),
        );

        let is_visible = ui.is_rect_visible(full_width_rect);
        if is_visible {
            let tree_has_data_in_current_timeline = entity_db.subtree_has_data_on_timeline(
                &entity_db.storage_engine(),
                time_ctrl.timeline_name(),
                entity_path,
            );
            if tree_has_data_in_current_timeline {
                let row_rect = Rect::from_x_y_ranges(
                    time_area_response.rect.x_range(),
                    response_rect.y_range(),
                );

                highlight_timeline_row(ui, ctx, time_area_painter, &item.to_item(), &row_rect);

                // show the density graph only if that item is closed
                if is_closed {
                    self.data_density_graph_ui(
                        ctx,
                        time_ctrl,
                        entity_db,
                        time_area_painter,
                        ui,
                        row_rect,
                        &item,
                        time_commands,
                    );
                }
            }
        }
    }

    /// Display the contents of an entity, i.e. its sub-entities and its components.
    #[expect(clippy::too_many_arguments)]
    fn show_entity_contents(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        viewport_blueprint: &ViewportBlueprint,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        entity_data: &EntityData,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        re_tracing::profile_function!();

        for child in &entity_data.children {
            self.show_entity(
                ctx,
                time_ctrl,
                viewport_blueprint,
                streams_tree_data,
                entity_db,
                time_area_response,
                time_area_painter,
                child,
                ui,
                time_commands,
            );
        }

        let entity_path = &entity_data.entity_path;
        let engine = entity_db.storage_engine();
        let store = engine.store();

        let components_by_archetype = components_for_entity(ctx, store, entity_path);
        let num_archetypes = components_by_archetype.len();
        for (archetype, components) in components_by_archetype {
            if archetype.is_none() && num_archetypes == 1 {
                // They are all without archetype, so we can skip the label.
            } else {
                let response = archetype_label_ui(ui, archetype);
                self.next_col_right = self.next_col_right.max(response.rect.right());
            }

            for component_descr in components {
                let component = component_descr.component;
                let is_static = store.entity_has_static_component(entity_path, component);

                let component_path = ComponentPath::new(entity_path.clone(), component);
                let item = TimePanelItem {
                    entity_path: entity_path.clone(),
                    component: Some(component),
                };
                let timeline = time_ctrl.timeline_name();

                let response = ui
                    .list_item()
                    .render_offscreen(false)
                    .selected(ctx.selection().contains_item(&item.to_item()))
                    .force_hovered(
                        ctx.selection_state()
                            .highlight_for_ui_element(&item.to_item())
                            == HoverHighlight::Hovered,
                    )
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new(component_descr.archetype_field_name())
                            .with_icon(if is_static {
                                &re_ui::icons::COMPONENT_STATIC
                            } else {
                                &re_ui::icons::COMPONENT_TEMPORAL
                            })
                            .truncate(false),
                    );

                self.handle_interactions_for_item(
                    ctx,
                    viewport_blueprint,
                    streams_tree_data,
                    entity_db,
                    item.to_item(),
                    &response,
                    false,
                );

                let response_rect = response.rect;

                response.on_hover_ui(|ui| {
                    let num_static_messages =
                        store.num_physical_static_events_for_component(entity_path, component);
                    let num_temporal_messages = store
                        .num_physical_temporal_events_for_component_on_timeline(
                            time_ctrl.timeline_name(),
                            entity_path,
                            component,
                        );
                    let total_num_messages = num_static_messages + num_temporal_messages;

                    if total_num_messages == 0 {
                        ui.label(
                            ui.ctx()
                                .warning_text(format!("No event logged on timeline {timeline:?}")),
                        );
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

                            ui.list_item()
                                .interactive(false)
                                .render_offscreen(false)
                                .show_flat(
                                    ui,
                                    list_item::LabelContent::new(format!(
                                        "{kind} {component} component, logged {num_messages}",
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
                                    *time_ctrl.timeline_name(),
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

                if is_visible {
                    let component_has_data_in_current_timeline = store
                        .entity_has_component_on_timeline(
                            time_ctrl.timeline_name(),
                            entity_path,
                            component,
                        );

                    if component_has_data_in_current_timeline {
                        // show the data in the time area:
                        let row_rect = Rect::from_x_y_ranges(
                            time_area_response.rect.x_range(),
                            response_rect.y_range(),
                        );

                        highlight_timeline_row(
                            ui,
                            ctx,
                            time_area_painter,
                            &item.to_item(),
                            &row_rect,
                        );

                        let db = match self.source {
                            TimePanelSource::Recording => ctx.recording(),
                            TimePanelSource::Blueprint => ctx.store_context.blueprint,
                        };

                        self.data_density_graph_ui(
                            ctx,
                            time_ctrl,
                            db,
                            time_area_painter,
                            ui,
                            row_rect,
                            &item,
                            time_commands,
                        );
                    }
                }
            }
        }
    }

    #[expect(clippy::too_many_arguments)]
    fn handle_interactions_for_item(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        item: Item,
        response: &egui::Response,
        is_draggable: bool,
    ) {
        context_menu_ui_for_item_with_context(
            ctx,
            viewport_blueprint,
            &item,
            // expand/collapse context menu actions need this information
            ItemContext::StreamsTree {
                store_kind: self.source.into(),
                filter_session_id: self.filter_state.session_id(),
            },
            response,
            SelectionUpdateBehavior::UseSelection,
        );
        ctx.handle_select_hover_drag_interactions(response, item.clone(), is_draggable);
        ctx.handle_select_focus_sync(response, item.clone());

        self.handle_range_selection(ctx, streams_tree_data, entity_db, item.clone(), response);

        if Some(item) == self.scroll_to_me_item {
            response.scroll_to_me(None);
            self.scroll_to_me_item = None;
        }
    }

    /// Paint a data density graph, supporting tooltips.
    #[expect(clippy::too_many_arguments)]
    fn data_density_graph_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        db: &re_entity_db::EntityDb,
        time_area_painter: &egui::Painter,
        ui: &egui::Ui,
        row_rect: Rect,
        item: &TimePanelItem,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let hovered_time = data_density_graph::data_density_graph_ui(
            &mut self.data_density_graph_painter,
            ctx,
            time_ctrl,
            db,
            time_area_painter,
            ui,
            &self.time_ranges_ui,
            row_rect,
            item,
        );

        if let Some(hovered_time) = hovered_time {
            self.hovered_event_time = Some(hovered_time);

            ctx.selection_state().set_hovered(item.to_item());

            if ui.input(|i| i.pointer.primary_clicked()) {
                ctx.command_sender()
                    .send_system(SystemCommand::SetSelection(item.to_item().into()));

                time_commands.push(TimeControlCommand::SetTime(hovered_time.into()));
            } else {
                ctx.selection_state().set_hovered(item.to_item());
            }

            if ui.ctx().dragged_id().is_none() {
                // TODO(jprochazk): check chunk.num_rows() and chunk.timeline.is_sorted()
                //                  if too many rows and unsorted, show some generic error tooltip (=too much data)
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    egui::Id::new("data_tooltip"),
                    egui::PopupAnchor::Pointer,
                )
                .gap(12.0)
                .show(|ui| {
                    data_density_graph::show_row_ids_tooltip(
                        ctx,
                        ui,
                        time_ctrl,
                        db,
                        item,
                        hovered_time,
                    );
                });
            }
        }
    }

    /// Handle setting/extending the item selection based on shift-clicking.
    ///
    /// NOTE: this is NOT the time range (loop) selection!
    fn handle_range_selection(
        &mut self,
        ctx: &ViewerContext<'_>,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        item: Item,
        response: &Response,
    ) {
        // Early out if we're not being clicked.
        if !response.clicked() {
            return;
        }

        let modifiers = ctx.egui_ctx().input(|i| i.modifiers);

        if modifiers.shift {
            if let Some(anchor_item) = &self.range_selection_anchor_item {
                let items_in_range = Self::items_in_range(
                    ctx,
                    streams_tree_data,
                    entity_db,
                    self.collapse_scope(),
                    anchor_item,
                    &item,
                );

                if items_in_range.is_empty() {
                    // This can happen if the last clicked item became invisible due to collapsing, or if
                    // the user switched to another recording. In either case, we invalidate it.
                    self.range_selection_anchor_item = None;
                } else {
                    let items = ItemCollection::from_items_and_context(
                        items_in_range.into_iter().map(|item| {
                            (
                                item,
                                Some(ItemContext::BlueprintTree {
                                    filter_session_id: self.filter_state.session_id(),
                                }),
                            )
                        }),
                    );

                    if modifiers.command {
                        // We extend into the current selection to append new items at the end.
                        let mut selection = ctx.selection().clone();
                        selection.extend(items);
                        ctx.command_sender()
                            .send_system(SystemCommand::set_selection(selection));
                    } else {
                        ctx.command_sender()
                            .send_system(SystemCommand::set_selection(items));
                    }
                }
            }
        } else {
            self.range_selection_anchor_item = Some(item);
        }
    }

    /// Selects a range of items in the streams tree.
    ///
    /// This method selects all [`Item`]s displayed between the provided shift-clicked item and the
    /// existing last-clicked item (if any). It takes into account the collapsed state, so only
    /// actually visible items may be selected.
    fn items_in_range(
        ctx: &ViewerContext<'_>,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        collapse_scope: CollapseScope,
        anchor_item: &Item,
        shift_clicked_item: &Item,
    ) -> Vec<Item> {
        let mut items_in_range = vec![];
        let mut found_last_clicked_items = false;
        let mut found_shift_clicked_items = false;

        let _ignored = streams_tree_data.visit(ctx, entity_db, |entity_or_component| {
            let item = entity_or_component.item();

            if &item == anchor_item {
                found_last_clicked_items = true;
            }

            if &item == shift_clicked_item {
                found_shift_clicked_items = true;
            }

            if found_last_clicked_items || found_shift_clicked_items {
                items_in_range.push(item);
            }

            if found_last_clicked_items && found_shift_clicked_items {
                return VisitorControlFlow::Break(());
            }

            let is_expanded = entity_or_component.is_open(ctx.egui_ctx(), collapse_scope);

            if is_expanded {
                VisitorControlFlow::Continue
            } else {
                VisitorControlFlow::SkipBranch
            }
        });

        if !found_last_clicked_items {
            vec![]
        } else {
            items_in_range
        }
    }

    fn top_row_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        entity_db: &re_entity_db::EntityDb,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        if ui.max_rect().width() < 600.0 {
            // Responsive ui for narrow screens, e.g. mobile. Split the controls into two rows.
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    self.time_control_ui
                        .play_pause_ui(time_ctrl, ui, time_commands);
                    self.time_control_ui
                        .playback_speed_ui(time_ctrl, ui, time_commands);
                    self.time_control_ui.fps_ui(time_ctrl, ui, time_commands);
                });
                ui.horizontal(|ui| {
                    self.time_control_ui.timeline_selector_ui(
                        time_ctrl,
                        entity_db.timeline_histograms(),
                        ui,
                        time_commands,
                    );

                    self.current_time_ui(ctx, time_ctrl, ui, time_commands);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        help_button(ui);
                    });
                });
            });
        } else {
            // One row:
            let timeline_histograms = entity_db.timeline_histograms();

            self.time_control_ui
                .play_pause_ui(time_ctrl, ui, time_commands);
            self.time_control_ui.timeline_selector_ui(
                time_ctrl,
                timeline_histograms,
                ui,
                time_commands,
            );
            self.time_control_ui
                .playback_speed_ui(time_ctrl, ui, time_commands);
            self.time_control_ui.fps_ui(time_ctrl, ui, time_commands);
            self.current_time_ui(ctx, time_ctrl, ui, time_commands);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                help_button(ui);

                let freshness = entity_db
                    .rrd_manifest_index()
                    .chunk_requests()
                    .bandwidth_data_freshness(ui.time());
                if ctx.app_options().show_metrics && freshness > 0.0 {
                    let mut rate = entity_db
                        .rrd_manifest_index()
                        .chunk_requests()
                        .bandwidth()
                        .unwrap_or(0.0);

                    if !rate.is_finite() {
                        rate = 0.0;
                    }

                    if 0.0 < rate {
                        ui.ctx().request_repaint(); // Show latest estimate
                    }

                    let staleness = 1.0 - freshness;
                    let gamma = 1.0 - staleness * staleness;
                    ui.label(
                        RichText::new(format!("{}/s", re_format::format_bytes(rate)))
                            .color(ui.style().visuals.text_color().gamma_multiply(gamma as f32)),
                    )
                    .on_hover_text("Connection throughput");
                }
            });
        }
    }

    fn collapse_scope(&self) -> CollapseScope {
        match (self.source, self.filter_state.session_id()) {
            (TimePanelSource::Recording, None) => CollapseScope::StreamsTree,

            (TimePanelSource::Blueprint, None) => CollapseScope::BlueprintStreamsTree,

            (TimePanelSource::Recording, Some(session_id)) => {
                CollapseScope::StreamsTreeFiltered { session_id }
            }

            (TimePanelSource::Blueprint, Some(session_id)) => {
                CollapseScope::BlueprintStreamsTreeFiltered { session_id }
            }
        }
    }

    fn collapsed_time_marker_and_time(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        entity_db: &re_entity_db::EntityDb,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        let Some(time_range) = entity_db.time_range_for(time_ctrl.timeline_name()) else {
            // We have no data on this timeline
            return;
        };

        if time_range.min() == time_range.max() {
            // Only one time point - showing a slider that can't be moved is just annoying
        } else {
            let space_needed_for_current_time = match time_ctrl.time_type() {
                Some(re_chunk_store::TimeType::Sequence) | None => 100.0,
                Some(re_chunk_store::TimeType::DurationNs) => 200.0,
                Some(re_chunk_store::TimeType::TimestampNs) => 220.0,
            };

            let mut time_range_rect = ui.available_rect_before_wrap();
            time_range_rect.max.x -= space_needed_for_current_time;

            let draw_loaded_ranges = false;
            data_density_graph::paint_loaded_indicator_bar(
                ui,
                &self.time_ranges_ui,
                entity_db,
                time_ctrl,
                time_range_rect.min.y,
                time_range_rect.x_range(),
                draw_loaded_ranges,
            );

            if time_range_rect.width() > 50.0 {
                ui.allocate_rect(time_range_rect, egui::Sense::hover());

                self.time_ranges_ui = initialize_time_ranges_ui(
                    time_ctrl,
                    entity_db,
                    time_range_rect.x_range(),
                    None,
                );
                self.time_ranges_ui
                    .snap_time_control(time_ctrl, time_commands);

                let painter = ui.painter_at(time_range_rect.expand(4.0));

                if let Some(highlighted_range) = time_ctrl.highlighted_range {
                    paint_range_highlight(
                        highlighted_range,
                        &self.time_ranges_ui,
                        &painter,
                        time_range_rect,
                    );
                }

                time_selection_ui::collapsed_loop_selection_ui(
                    time_ctrl,
                    &painter,
                    &self.time_ranges_ui,
                    ui,
                    time_range_rect,
                );

                // Show a centerline
                painter.hline(
                    time_range_rect.x_range(),
                    time_range_rect.center().y,
                    ui.visuals().widgets.noninteractive.fg_stroke,
                );

                data_density_graph::data_density_graph_ui(
                    &mut self.data_density_graph_painter,
                    ctx,
                    time_ctrl,
                    entity_db,
                    ui.painter(),
                    ui,
                    &self.time_ranges_ui,
                    time_range_rect.shrink2(egui::vec2(0.0, 10.0)),
                    &TimePanelItem::entity_path(EntityPath::root()),
                );

                self.time_marker_ui(
                    ui,
                    ctx,
                    time_ctrl,
                    None,
                    &painter,
                    &time_range_rect,
                    &time_range_rect,
                    time_commands,
                );
            }
        }

        self.current_time_ui(ctx, time_ctrl, ui, time_commands);
    }

    fn current_time_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        ui: &mut egui::Ui,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        if let Some(time_int) = time_ctrl.time_int()
            && let Some(time) = time_ctrl.time()
            && let Some(time_type) = time_ctrl.time_type()
        {
            /// Pick number of decimals to show based on zoom level
            ///
            /// The zoom level is expressed as nanoseconds per ui point (logical pixel).
            ///
            /// The formatting should omit trailing sub-second zeroes as far as `subsecond_decimals` perimts it.
            fn num_subsecond_decimals(nanos_per_point: f64) -> std::ops::RangeInclusive<usize> {
                if 1e9 < nanos_per_point {
                    0..=6
                } else if 1e8 < nanos_per_point {
                    1..=6
                } else if 1e6 < nanos_per_point {
                    3..=6
                } else if 1e3 < nanos_per_point {
                    6..=9
                } else {
                    9..=9
                }
            }

            let subsecond_decimals =
                num_subsecond_decimals(1.0 / self.time_ranges_ui.points_per_time);

            let mut time_str = self.time_edit_string.clone().unwrap_or_else(|| {
                time_type.format_opt(
                    time_int,
                    ctx.app_options().timestamp_format,
                    subsecond_decimals,
                )
            });

            ui.style_mut().spacing.text_edit_width = 200.0;

            let response = ui.text_edit_singleline(&mut time_str);
            if response.changed() {
                self.time_edit_string = Some(time_str.clone());
            }
            if response.lost_focus() {
                if let Some(time_int) =
                    time_type.parse_time(&time_str, ctx.app_options().timestamp_format)
                {
                    time_commands.push(TimeControlCommand::SetTime(time_int.into()));
                } else {
                    re_log::warn!("Failed to parse {time_str:?}");
                }
                self.time_edit_string = None;
            }
            let response = response.on_hover_text(format!(
                "Timestamp: {}",
                re_format::format_int(time_int.as_i64())
            ));

            response.context_menu(|ui| {
                copy_time_properties_context_menu(ui, time);
            });
        }
    }
}

fn archetype_label_ui(
    ui: &mut Ui,
    archetype: Option<re_sdk_types::ArchetypeName>,
) -> egui::Response {
    ui.list_item()
        .with_y_offset(1.0)
        .with_height(20.0)
        .interactive(false)
        .show_hierarchical(
            ui,
            list_item::LabelContent::new(
                RichText::new(
                    archetype
                        .map(|a| a.short_name())
                        .unwrap_or("Without archetype"),
                )
                .size(10.0),
            ),
        )
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
        painter.rect_filled(*row_rect, egui::CornerRadius::ZERO, bg_color);
    }
}

fn paint_range_highlight(
    highlighted_range: AbsoluteTimeRange,
    time_ranges_ui: &TimeRangesUi,
    painter: &egui::Painter,
    rect: Rect,
) {
    time_selection_ui::paint_timeline_range(
        highlighted_range,
        time_ranges_ui,
        painter,
        rect,
        painter.ctx().tokens().extreme_fg_color.gamma_multiply(0.1),
    );
}

fn help(os: egui::os::OperatingSystem) -> Help {
    // There are multiple ways to pan and zoom:
    // Mac trackpad: swipe and pinch
    // Mouse: Scroll with shift/command
    // Mouse: Drag with secondary/middle
    // Which should we show here?
    // If you have a good trackpad, we could hide the other ways to pan/zoom.
    // But how can we know?
    // Should we just assume that every mac user has a trackpad, and nobody else does?
    // But some mac users (like @Wumpf) use a mouse with their mac.
    Help::new("Timeline")
        .control("Select time segment", "Drag time scale")
        .control("Snap to grid", icons::SHIFT)
        .control("Pan", "Middle click drag")
        .control("Pan vertically", "Scroll")
        .control(
            "Pan horizontally",
            (IconText::from_modifiers(os, Modifiers::SHIFT), " + Scroll"),
        )
        .control(
            "Zoom",
            (
                IconText::from_modifiers(os, Modifiers::COMMAND),
                " + Scroll",
            ),
        )
        .control("Zoom", "Right click drag")
        .control("Reset view", "Double click")
        .control("Play/Pause", "Space")
}

fn help_button(ui: &mut egui::Ui) {
    ui.help_button(|ui| {
        help(ui.ctx().os()).ui(ui);
    });
}

// ----------------------------------------------------------------------------

fn initialize_time_ranges_ui(
    time_ctrl: &TimeControl,
    entity_db: &re_entity_db::EntityDb,
    x_range: Rangef,
    mut time_view: Option<TimeView>,
) -> TimeRangesUi {
    re_tracing::profile_function!();

    let mut time_range = Vec::new();

    let timeline = time_ctrl.timeline_name();
    if let Some(times) = entity_db.time_histogram(timeline)
        && let Some(time_type) = time_ctrl.time_type()
    {
        // NOTE: `times` can be empty if a GC wiped everything.
        if !times.is_empty() {
            let timeline_axis = TimelineAxis::new(time_type, times);
            time_view = time_view.or_else(|| Some(view_everything(&x_range, &timeline_axis)));
            time_range.extend(timeline_axis.ranges);
        }
    }

    TimeRangesUi::new(
        x_range,
        time_view.unwrap_or_else(|| TimeView {
            min: TimeReal::from(0),
            time_spanned: 1.0,
        }),
        &time_range,
    )
}

/// Find a nice view of everything in the valid marked range.
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

    let min_data_time = timeline_axis.ranges.first().min;
    let min_valid_data_time = min_data_time;
    let time_spanned = timeline_axis.sum_time_lengths() as f64 * factor as f64;

    TimeView {
        min: min_valid_data_time.into(),
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
            shadow_mesh.colored_vertex(right_pos, ui.tokens().shadow_gradient_dark_start);

            left_line_strip.push(left_pos);
            right_line_strip.push(right_pos);

            y += zig_height;
            row += 1;
        }

        // Regular & shadow mesh have the same topology!
        shadow_mesh.indices.clone_from(&mesh.indices);

        painter.add(Shape::Mesh(Arc::new(mesh)));
        painter.add(Shape::Mesh(Arc::new(shadow_mesh)));
        painter.add(Shape::line(left_line_strip, stroke));
        painter.add(Shape::line(right_line_strip, stroke));
    };

    let zig_zag_first_and_last_edges = true;

    // Margin for the (left or right) end of a gap.
    // Don't use an arbitrarily large value since it can cause platform-specific rendering issues.
    const GAP_END_MARGIN: f32 = 100.0;

    if let Some(segment) = time_ranges_ui.segments.first() {
        let gap_edge = *segment.x.start() as f32;
        let gap_edge_left_side = ui.ctx().content_rect().left() - GAP_END_MARGIN;

        if zig_zag_first_and_last_edges {
            // Left side of first segment - paint as a very wide gap that we only see the right side of
            paint_time_gap(gap_edge_left_side, gap_edge);
        } else {
            // Careful with subtracting a too large number here. Nvidia @ Windows was observed not drawing the rect correctly for -100_000.0
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge - 10_000.0, top), pos2(gap_edge, bottom)),
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
        let gap_edge_right_side = ui.ctx().content_rect().right() + GAP_END_MARGIN;

        if zig_zag_first_and_last_edges {
            // Right side of last segment - paint as a very wide gap that we only see the left side of
            paint_time_gap(gap_edge, gap_edge_right_side);
        } else {
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge, top), pos2(gap_edge_right_side, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range, stroke);
        }
    }
}

/// Handle zooming, panning, etc.
///
/// Does NOT handle moving the time cursor.
#[must_use]
fn pan_and_zoom_interaction(
    time_ranges_ui: &TimeRangesUi,
    ui: &egui::Ui,
    full_rect: &Rect,
    streams_rect: &Rect,
    time_commands: &mut Vec<TimeControlCommand>,
) -> egui::Response {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    let mut delta_x = 0.0;
    let mut zoom_factor = 1.0;

    // Check for zoom/pan inputs (via e.g. horizontal scrolling) on the entire
    // time area rectangle, including the timeline rect.
    let full_rect_hovered = pointer_pos.is_some_and(|pointer_pos| full_rect.contains(pointer_pos));
    if full_rect_hovered {
        ui.input(|input| {
            delta_x += input.smooth_scroll_delta.x;
            zoom_factor *= input.zoom_delta_2d().x;
        });
    }

    // We only check for drags in the streams rect, because
    // drags in the timeline rect should create loop selections.
    let response = ui.interact(
        *streams_rect,
        ui.id().with("time_area_interact"),
        egui::Sense::click_and_drag(),
    );

    if response.dragged_by(PointerButton::Secondary) {
        zoom_factor *= (response.drag_delta().y * 0.01).exp();
    }

    if response.dragged_by(PointerButton::Middle) {
        delta_x += response.drag_delta().x;
        ui.ctx().set_cursor_icon(CursorIcon::AllScroll);
    }

    if delta_x != 0.0
        && let Some(new_view_range) = time_ranges_ui.pan(-delta_x)
    {
        time_commands.push(TimeControlCommand::SetTimeView(new_view_range));
    }

    if zoom_factor != 1.0
        && let Some(pointer_pos) = pointer_pos
        && let Some(new_view_range) = time_ranges_ui.zoom_at(pointer_pos.x, zoom_factor)
    {
        time_commands.push(TimeControlCommand::SetTimeView(new_view_range));
    }

    if response.double_clicked() {
        time_commands.push(TimeControlCommand::ResetTimeView);
    }

    response
}

/// Context menu that shows up when interacting with the streams rect.
fn timeline_properties_context_menu(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    time_ctrl: &TimeControl,
    hovered_time: TimeReal,
) {
    let mut url = ViewerOpenUrl::from_context(ctx);
    let has_fragment = url.as_mut().is_ok_and(|url| {
        if let Some(fragment) = url.fragment_mut() {
            fragment.time_selection = None;
            fragment.when = time_ctrl.time_type().map(|typ| {
                (
                    *time_ctrl.timeline_name(),
                    re_log_types::TimeCell {
                        typ,
                        value: hovered_time.floor().into(),
                    },
                )
            });
            true
        } else {
            false
        }
    });
    let copy_command = url.and_then(|url| url.copy_url_command());

    if ui
        .add_enabled(
            copy_command.is_ok() && has_fragment,
            egui::Button::new("Copy link to timestamp"),
        )
        .on_disabled_hover_text(if let Err(err) = copy_command.as_ref() {
            format!("Can't share links to the current recording: {err}")
        } else {
            "The current recording doesn't support time stamp links".to_owned()
        })
        .clicked()
        && let Ok(copy_command) = copy_command
    {
        ctx.command_sender().send_system(copy_command);
    }

    if ui.button("Copy timestamp").clicked() {
        let time = format!("{}", hovered_time.floor().as_i64());
        re_log::info!("Copied hovered timestamp: {}", time);
        ui.ctx().copy_text(time);
    }
}

fn copy_time_properties_context_menu(ui: &mut egui::Ui, time: TimeReal) {
    if ui.button("Copy timestamp").clicked() {
        let time = format!("{}", time.floor().as_i64());
        re_log::info!("Copied hovered timestamp: {}", time);
        ui.ctx().copy_text(time);
    }
}

impl TimePanel {
    /// A vertical line that shows the current time.
    ///
    /// This function both paints it and allows click and drag to interact with the current time.
    #[expect(clippy::too_many_arguments)]
    fn time_marker_ui(
        &self,
        ui: &egui::Ui,
        ctx: &ViewerContext<'_>,
        time_ctrl: &TimeControl,
        time_area_response: Option<egui::Response>,
        time_area_painter: &egui::Painter,
        timeline_rect: &Rect,
        interact_rect: &Rect,
        time_commands: &mut Vec<TimeControlCommand>,
    ) {
        // timeline_rect: top part with the second ticks and time marker

        // We only check for drags in the streams rect, because
        // drags in the timeline rect should create loop selections.
        let response = time_area_response
            .unwrap_or_else(|| {
                ui.interact(
                    *interact_rect,
                    ui.id().with("time_cursor_interact"),
                    egui::Sense::click_and_drag(),
                )
            })
            .on_hover_cursor(MOVE_TIME_CURSOR_ICON);

        let hovered_time = self.hovered_event_time.map(TimeReal::from).or_else(|| {
            let pointer_pos = response.hover_pos()?;
            self.time_ranges_ui.snapped_time_from_x(ui, pointer_pos.x)
        });

        // Press to move time:
        if ui.input(|i| i.pointer.primary_pressed() || i.pointer.primary_down() || i.pointer.primary_released())
            // `interact_pointer_pos` is set as soon as the mouse button is down on it,
            // without having to wait for the drag to go far enough or long enough
            && response.interact_pointer_pos().is_some()
            && let Some(time) = hovered_time
        {
            time_commands.push(TimeControlCommand::SetTime(time));
        }

        // Show hover preview, and right-click context menu:
        {
            let right_clicked_time_id = egui::Id::new("__right_clicked_time");

            let right_clicked_time = ui
                .ctx()
                .memory(|mem| mem.data.get_temp(right_clicked_time_id));

            // If we have right-clicked a time, we show it, else the hovered time.
            let preview_time = right_clicked_time.or(hovered_time);

            if let Some(preview_time) = preview_time {
                let preview_x = self.time_ranges_ui.x_from_time_f32(preview_time);

                if let Some(preview_x) = preview_x {
                    time_area_painter.vline(
                        preview_x,
                        timeline_rect.top()..=ui.max_rect().bottom(),
                        ui.visuals().widgets.noninteractive.fg_stroke,
                    );
                }

                let popup_is_open = egui::Popup::context_menu(&response)
                    .width(300.0)
                    .show(|ui| {
                        timeline_properties_context_menu(ui, ctx, time_ctrl, preview_time);
                    })
                    .is_some();
                if popup_is_open {
                    ui.ctx().memory_mut(|mem| {
                        mem.data.insert_temp(right_clicked_time_id, preview_time);
                    });
                } else {
                    ui.ctx()
                        .memory_mut(|mem| mem.data.remove::<TimeReal>(right_clicked_time_id));
                }
            }
        }

        // Paint current time:
        {
            // Use latest available time to avoid frame delay:
            let mut current_time = time_ctrl.time();
            for cmd in time_commands {
                if let TimeControlCommand::SetTime(time) = cmd {
                    current_time = Some(*time);
                }
            }

            if let Some(time) = current_time
                && let Some(x) = self.time_ranges_ui.x_from_time_f32(time)
                && timeline_rect.x_range().contains(x)
            {
                ui.paint_time_cursor(
                    time_area_painter,
                    None,
                    x,
                    Rangef::new(timeline_rect.top(), ui.max_rect().bottom()),
                );
            }
        };
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(help);
}
