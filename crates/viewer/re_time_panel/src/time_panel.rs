use std::ops::ControlFlow;
use std::sync::Arc;

use egui::emath::Rangef;
use egui::{
    Color32, CursorIcon, Modifiers, NumExt as _, Painter, PointerButton, Rect, Response, Shape, Ui,
    Vec2, pos2, scroll_area::ScrollSource,
};

use re_context_menu::{SelectionUpdateBehavior, context_menu_ui_for_item_with_context};
use re_data_ui::DataUi as _;
use re_data_ui::item_ui::guess_instance_path_icon;
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{
    ApplicationId, ComponentPath, EntityPath, ResolvedTimeRange, TimeInt, TimeReal,
};
use re_types::blueprint::components::PanelState;
use re_types_core::ComponentDescriptor;
use re_ui::filter_widget::format_matching_text;
use re_ui::{
    ContextExt as _, DesignTokens, Help, SyntaxHighlighting as _, UiExt as _, filter_widget,
    icon_text, icons, list_item, maybe_plus, modifiers_text,
};
use re_viewer_context::{
    CollapseScope, HoverHighlight, Item, ItemContext, RecordingConfig, TimeControl, TimeView,
    UiLayout, ViewerContext, VisitorControlFlow,
};
use re_viewport_blueprint::ViewportBlueprint;

use crate::{
    recursive_chunks_per_timeline_subscriber::PathRecursiveChunksPerTimelineStoreSubscriber,
    streams_tree_data::{EntityData, StreamsTreeData, components_for_entity},
    time_axis::TimelineAxis,
    time_control_ui::TimeControlUi,
    time_ranges_ui::TimeRangesUi,
    {data_density_graph, paint_ticks, time_ranges_ui, time_selection_ui},
};

#[derive(Debug, Clone)]
pub struct TimePanelItem {
    pub entity_path: EntityPath,
    pub component_descr: Option<ComponentDescriptor>,
}

impl TimePanelItem {
    pub fn entity_path(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            component_descr: None,
        }
    }

    pub fn to_item(&self) -> Item {
        let Self {
            entity_path,
            component_descr,
        } = self;

        if let Some(component_descr) = component_descr.as_ref() {
            Item::ComponentPath(ComponentPath::new(
                entity_path.clone(),
                component_descr.clone(),
            ))
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
    scroll_to_me_item: Option<Item>,

    /// If the timestamp is being edited, the current value.
    ///
    /// It is applied only after removing focus.
    #[serde(skip)]
    pub time_edit_string: Option<String>,
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

        // Invalidate the filter widget if the store id has changed.
        if self.filter_state_app_id.as_ref() != Some(&ctx.store_context.app_id) {
            self.filter_state = Default::default();
            self.filter_state_app_id = Some(ctx.store_context.app_id.clone());
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
            panel_frame.inner_margin.bottom = 0;

            // Similarly, let the data get close to the right edge:
            panel_frame.inner_margin.right = 0;
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

        let time_range = entity_db.time_range_for(time_ctrl.timeline().name());
        let has_more_than_one_time_point =
            time_range.is_some_and(|time_range| time_range.min() != time_range.max());

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
                    self.collapsed_time_marker_and_time(ui, ctx, entity_db, time_ctrl);
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

            self.collapsed_time_marker_and_time(ui, ctx, entity_db, time_ctrl);
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

            ui.add_space(-4.0); // hack to vertically center the text

            let size = egui::vec2(self.prev_col_width, 27.0);
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
            ctx.app_options().timestamp_format,
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
    #[expect(clippy::too_many_arguments)]
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
            // We turn off `ScrollSource::DRAG` so that the `ScrollArea` don't steal input from
            // the earlier `interact_with_time_area`.
            // We implement drag-to-scroll manually instead!
            .scroll_source(ScrollSource::MOUSE_WHEEL | ScrollSource::SCROLL_BAR)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0; // no spacing needed for ListItems

                if time_area_response.dragged_by(PointerButton::Primary) {
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
                        viewport_blueprint,
                        &streams_tree_data,
                        entity_db,
                        time_ctrl,
                        time_area_response,
                        time_area_painter,
                        child,
                        ui,
                    );
                }
            });
    }

    /// Display the list item for an entity.
    #[expect(clippy::too_many_arguments)]
    fn show_entity(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &mut TimeControl,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        entity_data: &EntityData,
        ui: &mut egui::Ui,
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
            .focused_item
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
                        viewport_blueprint,
                        streams_tree_data,
                        entity_db,
                        time_ctrl,
                        time_area_response,
                        time_area_painter,
                        entity_data,
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
                time_ctrl.timeline().name(),
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
                    data_density_graph::data_density_graph_ui(
                        &mut self.data_density_graph_painter,
                        ctx,
                        time_ctrl,
                        entity_db,
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

    /// Display the contents of an entity, i.e. its sub-entities and its components.
    #[expect(clippy::too_many_arguments)]
    fn show_entity_contents(
        &mut self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &mut TimeControl,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        entity_data: &EntityData,
        ui: &mut egui::Ui,
    ) {
        re_tracing::profile_function!();

        for child in &entity_data.children {
            self.show_entity(
                ctx,
                viewport_blueprint,
                streams_tree_data,
                entity_db,
                time_ctrl,
                time_area_response,
                time_area_painter,
                child,
                ui,
            );
        }

        let entity_path = &entity_data.entity_path;
        let engine = entity_db.storage_engine();
        let store = engine.store();

        for component_descr in components_for_entity(store, entity_path) {
            let is_static = store.entity_has_static_component(entity_path, &component_descr);

            let component_path = ComponentPath::new(entity_path.clone(), component_descr);
            let component_descr = &component_path.component_descriptor;
            let item = TimePanelItem {
                entity_path: entity_path.clone(),
                component_descr: Some(component_descr.clone()),
            };
            let timeline = time_ctrl.timeline();

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
                    list_item::LabelContent::new(component_descr.syntax_highlighted(ui.style()))
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
                    store.num_static_events_for_component(entity_path, component_descr);
                let num_temporal_messages = store.num_temporal_events_for_component_on_timeline(
                    time_ctrl.timeline().name(),
                    entity_path,
                    component_descr,
                );
                let total_num_messages = num_static_messages + num_temporal_messages;

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

                        ui.list_item()
                            .interactive(false)
                            .render_offscreen(false)
                            .show_flat(
                                ui,
                                list_item::LabelContent::new(format!(
                                    "{kind} {} component, logged {num_messages}",
                                    component_descr.component_name.short_name()
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
                                *time_ctrl.timeline().name(),
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
                        time_ctrl.timeline().name(),
                        entity_path,
                        component_descr,
                    );

                if component_has_data_in_current_timeline {
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

        self.handle_range_selection(ctx, streams_tree_data, entity_db, item.clone(), response);

        self.handle_key_navigation(ctx, streams_tree_data, entity_db, &item);

        if Some(item) == self.scroll_to_me_item {
            response.scroll_to_me(None);
            self.scroll_to_me_item = None;
        }
    }

    fn handle_key_navigation(
        &mut self,
        ctx: &ViewerContext<'_>,
        streams_tree_data: &StreamsTreeData,
        entity_db: &re_entity_db::EntityDb,
        item: &Item,
    ) {
        if ctx.selection_state().selected_items().single_item() != Some(item) {
            return;
        }

        if ctx
            .egui_ctx()
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
        {
            if let Some(collapse_id) = self.collapse_scope().item(item.clone()) {
                collapse_id.set_open(ctx.egui_ctx(), true);
            }
        }

        if ctx
            .egui_ctx()
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
        {
            if let Some(collapse_id) = self.collapse_scope().item(item.clone()) {
                collapse_id.set_open(ctx.egui_ctx(), false);
            }
        }

        if ctx
            .egui_ctx()
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        {
            let mut found_current = false;

            let result = streams_tree_data.visit(entity_db, |entity_or_component| {
                let tree_item = entity_or_component.item();
                let is_item_collapsed =
                    !entity_or_component.is_open(ctx.egui_ctx(), self.collapse_scope());

                if &tree_item == item {
                    found_current = true;

                    return if is_item_collapsed {
                        VisitorControlFlow::SkipBranch
                    } else {
                        VisitorControlFlow::Continue
                    };
                }

                if found_current {
                    VisitorControlFlow::Break(Some(tree_item))
                } else if is_item_collapsed {
                    VisitorControlFlow::SkipBranch
                } else {
                    VisitorControlFlow::Continue
                }
            });

            if let ControlFlow::Break(Some(item)) = result {
                ctx.selection_state().set_selection(item.clone());
                self.scroll_to_me_item = Some(item.clone());
                self.range_selection_anchor_item = Some(item);
            }
        }

        if ctx
            .egui_ctx()
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
        {
            let mut last_item = None;

            let result = streams_tree_data.visit(entity_db, |entity_or_component| {
                let tree_item = entity_or_component.item();
                let is_item_collapsed =
                    !entity_or_component.is_open(ctx.egui_ctx(), self.collapse_scope());

                if &tree_item == item {
                    return VisitorControlFlow::Break(last_item.clone());
                }

                last_item = Some(tree_item);

                if is_item_collapsed {
                    VisitorControlFlow::SkipBranch
                } else {
                    VisitorControlFlow::Continue
                }
            });

            if let ControlFlow::Break(Some(item)) = result {
                ctx.selection_state().set_selection(item.clone());
                self.scroll_to_me_item = Some(item.clone());
                self.range_selection_anchor_item = Some(item);
            }
        }
    }

    /// Handle setting/extending the selection based on shift-clicking.
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
                    let items_iterator = items_in_range.into_iter().map(|item| {
                        (
                            item,
                            Some(ItemContext::BlueprintTree {
                                filter_session_id: self.filter_state.session_id(),
                            }),
                        )
                    });

                    if modifiers.command {
                        ctx.selection_state.extend_selection(items_iterator);
                    } else {
                        ctx.selection_state.set_selection(items_iterator);
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

        streams_tree_data.visit(entity_db, |entity_or_component| {
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

                    self.current_time_ui(ctx, ui, time_ctrl);

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
            self.current_time_ui(ctx, ui, time_ctrl);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                help_button(ui);
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
        entity_db: &re_entity_db::EntityDb,
        time_ctrl: &mut TimeControl,
    ) {
        let timeline = time_ctrl.timeline();

        let Some(time_range) = entity_db.time_range_for(timeline.name()) else {
            // We have no data on this timeline
            return;
        };

        if time_range.min() == time_range.max() {
            // Only one time point - showing a slider that can't be moved is just annoying
        } else {
            let space_needed_for_current_time = match timeline.typ() {
                re_chunk_store::TimeType::Sequence => 100.0,
                re_chunk_store::TimeType::DurationNs => 200.0,
                re_chunk_store::TimeType::TimestampNs => 220.0,
            };

            let mut time_range_rect = ui.available_rect_before_wrap();
            time_range_rect.max.x -= space_needed_for_current_time;

            if time_range_rect.width() > 50.0 {
                ui.allocate_rect(time_range_rect, egui::Sense::hover());

                let time_ranges_ui = initialize_time_ranges_ui(
                    entity_db,
                    time_ctrl,
                    time_range_rect.x_range(),
                    None,
                );
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
                    &mut self.data_density_graph_painter,
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

        self.current_time_ui(ctx, ui, time_ctrl);
    }

    fn current_time_ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        time_ctrl: &mut TimeControl,
    ) {
        if let Some(time_int) = time_ctrl.time_int() {
            let time_type = time_ctrl.time_type();

            let mut time_str = self
                .time_edit_string
                .clone()
                .unwrap_or_else(|| time_type.format(time_int, ctx.app_options().timestamp_format));

            ui.style_mut().spacing.text_edit_width = 200.0;

            let response = ui.text_edit_singleline(&mut time_str);
            if response.changed() {
                self.time_edit_string = Some(time_str.clone());
            }
            if response.lost_focus() {
                if let Some(time_int) =
                    time_type.parse_time(&time_str, ctx.app_options().timestamp_format)
                {
                    time_ctrl.set_time(time_int);
                }
                self.time_edit_string = None;
            }
            response
                .on_hover_text(format!("Timestamp: {}", time_int.as_i64()))
                .context_menu(|ui| {
                    copy_time_properties_context_menu(ui, time_ctrl, None);
                });
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
        painter.rect_filled(*row_rect, egui::CornerRadius::ZERO, bg_color);
    }
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

        painter.rect_filled(
            visible_history_area_rect,
            0.0,
            painter
                .ctx()
                .design_tokens()
                .extreme_fg_color
                .gamma_multiply(0.1),
        );
    }
}

fn help(ctx: &egui::Context) -> Help {
    Help::new("Timeline")
        .control("Play/Pause", "Space")
        .control(
            "Move time cursor",
            icon_text!(icons::LEFT_MOUSE_CLICK, "+", "drag time scale"),
        )
        .control(
            "Select time segment",
            icon_text!(icons::SHIFT, "+", "drag time scale"),
        )
        .control(
            "Pan",
            icon_text!(icons::LEFT_MOUSE_CLICK, "+", "drag event canvas"),
        )
        .control(
            "Zoom",
            icon_text!(
                modifiers_text(Modifiers::COMMAND, ctx),
                maybe_plus(ctx),
                icons::SCROLL
            ),
        )
        .control("Reset view", icon_text!("double", icons::LEFT_MOUSE_CLICK))
}

fn help_button(ui: &mut egui::Ui) {
    ui.help_hover_button().on_hover_ui(|ui| {
        help(ui.ctx()).ui(ui);
    });
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

    if let Some(times) = entity_db.time_histogram(time_ctrl.timeline().name()) {
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
            shadow_mesh.colored_vertex(right_pos, ui.design_tokens().shadow_gradient_dark_start);

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
    let full_rect_hovered = pointer_pos.is_some_and(|pointer_pos| full_rect.contains(pointer_pos));
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

/// Context menu that shows up when interacting with the streams rect.
fn copy_time_properties_context_menu(
    ui: &mut egui::Ui,
    time_ctrl: &TimeControl,
    hovered_time: Option<TimeReal>,
) {
    if let Some(time) = hovered_time {
        if ui.button("Copy hovered timestamp").clicked() {
            let time = format!("{}", time.floor().as_i64());
            re_log::info!("Copied hovered timestamp: {}", time);
            ui.ctx().copy_text(time);
        };
    } else if let Some(time) = time_ctrl.time_int() {
        if ui.button("Copy current timestamp").clicked() {
            let time = format!("{}", time.as_i64());
            re_log::info!("Copied current timestamp: {}", time);
            ui.ctx().copy_text(time);
        };
    }

    if ui.button("Copy current timeline name").clicked() {
        let timeline = format!("{}", time_ctrl.timeline().name());
        re_log::info!("Copied current timeline: {}", timeline);
        ui.ctx().copy_text(timeline);
    }
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

        let hovered_time = time_ranges_ui.time_from_x_f32(pointer_pos.x);

        if !is_hovering_the_loop_selection {
            let mut set_time_to_pointer = || {
                if let Some(time) = hovered_time {
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

        time_area_response
            .context_menu(|ui| copy_time_properties_context_menu(ui, time_ctrl, hovered_time));
    }
}

#[test]
fn test_help_view() {
    re_viewer_context::test_context::TestContext::test_help_view(help);
}
