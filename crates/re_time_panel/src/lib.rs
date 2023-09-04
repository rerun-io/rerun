//! Rerun Time Panel
//!
//! This crate provides a panel that shows allows to control time & timelines,
//! as well as all necessary ui elements that make it up.

mod data_density_graph;
mod paint_ticks;
mod time_axis;
mod time_control_ui;
mod time_ranges_ui;
mod time_selection_ui;

use egui::emath::Rangef;
use egui::{pos2, Color32, CursorIcon, NumExt, Painter, PointerButton, Rect, Shape, Ui, Vec2};
use std::slice;

use re_data_store::{EntityTree, InstancePath, TimeHistogram};
use re_data_ui::item_ui;
use re_log_types::{ComponentPath, EntityPathPart, TimeInt, TimeRange, TimeReal};
use re_ui::list_item::{ListItem, WidthAllocationMode};
use re_viewer_context::{HoverHighlight, Item, TimeControl, TimeView, ViewerContext};

use time_axis::TimelineAxis;
use time_control_ui::TimeControlUi;
use time_ranges_ui::TimeRangesUi;

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
}

impl Default for TimePanel {
    fn default() -> Self {
        Self {
            data_density_graph_painter: Default::default(),
            prev_col_width: 400.0,
            next_col_right: 0.0,
            time_ranges_ui: Default::default(),
            time_control_ui: TimeControlUi,
        }
    }
}

impl TimePanel {
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        time_panel_expanded: bool,
    ) {
        // this is the size of everything above the central panel (window title bar, top bar on web,
        // etc.)
        let screen_header_height = ui.cursor().top();

        let top_bar_height = 28.0;
        let margin = ctx.re_ui.bottom_panel_margin();
        let mut panel_frame = ctx.re_ui.bottom_panel_frame();

        if time_panel_expanded {
            // Since we use scroll bars we want to fill the whole vertical space downwards:
            panel_frame.inner_margin.bottom = 0.0;

            // Similarly, let the data get close to the right edge:
            panel_frame.inner_margin.right = 0.0;
        }

        let window_height = ui.ctx().screen_rect().height();

        let collapsed = egui::TopBottomPanel::bottom("time_panel_collapsed")
            .resizable(false)
            .show_separator_line(false)
            .frame(panel_frame)
            .default_height(44.0);

        let min_height = 150.0;
        let min_top_space = 150.0 + screen_header_height;
        let expanded = egui::TopBottomPanel::bottom("time_panel_expanded")
            .resizable(true)
            .show_separator_line(false)
            .frame(panel_frame)
            .min_height(min_height)
            .max_height((window_height - min_top_space).at_least(min_height).round())
            .default_height((0.25 * window_height).clamp(min_height, 250.0).round());

        egui::TopBottomPanel::show_animated_between_inside(
            ui,
            time_panel_expanded,
            collapsed,
            expanded,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed or animating
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size = Vec2::splat(top_bar_height);
                        ui.visuals_mut().button_frame = true;
                        self.collapsed_ui(ctx, ui);
                    });
                } else {
                    // Expanded:
                    ui.vertical(|ui| {
                        // Add back the margin we removed from the panel:
                        let mut top_row_frame = egui::Frame::default();
                        top_row_frame.inner_margin.right = margin.x;
                        top_row_frame.inner_margin.bottom = margin.y;
                        let top_row_rect = top_row_frame
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().interact_size = Vec2::splat(top_bar_height);
                                    ui.visuals_mut().button_frame = true;
                                    self.top_row_ui(ctx, ui);
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

                        ui.spacing_mut().scroll_bar_outer_margin = 4.0; // needed, because we have no panel margin on the right side.

                        // Add extra margin on the left which was intentionally missing on the controls.
                        let mut streams_frame = egui::Frame::default();
                        streams_frame.inner_margin.left = margin.x;
                        streams_frame.show(ui, |ui| {
                            self.expanded_ui(ctx, ui);
                        });
                    });
                }
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn collapsed_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        if ui.max_rect().width() < 600.0 {
            // Responsive ui for narrow screens, e.g. mobile. Split the controls into two rows.
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let re_ui = &ctx.re_ui;
                    let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
                    let times_per_timeline = ctx.store_db.times_per_timeline();
                    self.time_control_ui
                        .play_pause_ui(time_ctrl, re_ui, times_per_timeline, ui);
                    self.time_control_ui.playback_speed_ui(time_ctrl, ui);
                    self.time_control_ui.fps_ui(time_ctrl, ui);
                });
                ui.horizontal(|ui| {
                    let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
                    self.time_control_ui.timeline_selector_ui(
                        time_ctrl,
                        ctx.store_db.times_per_timeline(),
                        ui,
                    );
                    collapsed_time_marker_and_time(ui, ctx);
                });
            });
        } else {
            // One row:
            let re_ui = &ctx.re_ui;
            let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
            let times_per_timeline = ctx.store_db.times_per_timeline();
            self.time_control_ui
                .play_pause_ui(time_ctrl, re_ui, times_per_timeline, ui);
            self.time_control_ui
                .timeline_selector_ui(time_ctrl, times_per_timeline, ui);
            self.time_control_ui.playback_speed_ui(time_ctrl, ui);
            self.time_control_ui.fps_ui(time_ctrl, ui);

            collapsed_time_marker_and_time(ui, ctx);
        }
    }

    fn expanded_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        self.data_density_graph_painter.begin_frame(ui.ctx());

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
            let right = ui.max_rect().right() - ui.spacing_mut().scroll_bar_outer_margin;
            debug_assert!(time_x_left < right);
            Rangef::new(time_x_left, right)
        };

        let side_margin = 26.0; // chosen so that the scroll bar looks approximately centered in the default gap
        self.time_ranges_ui = initialize_time_ranges_ui(
            ctx,
            Rangef::new(
                time_fg_x_range.min + side_margin,
                time_fg_x_range.max - side_margin,
            ),
            ctx.rec_cfg.time_ctrl.time_view(),
        );
        let full_y_range = Rangef::new(ui.min_rect().bottom(), ui.max_rect().bottom());

        let timeline_rect = {
            let top = ui.min_rect().bottom();

            let size = egui::vec2(self.prev_col_width, 28.0);
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_min_size(size);
                ui.style_mut().wrap = Some(false);
                ui.add_space(4.0); // hack to vertically center the text
                ui.strong("Streams");
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
            ctx.rec_cfg.time_ctrl.time_type(),
        );
        paint_time_ranges_gaps(
            &self.time_ranges_ui,
            ctx.re_ui,
            ui,
            &time_bg_area_painter,
            full_y_range,
        );
        time_selection_ui::loop_selection_ui(
            ctx.store_db,
            &mut ctx.rec_cfg.time_ctrl,
            &self.time_ranges_ui,
            ui,
            &time_bg_area_painter,
            &timeline_rect,
        );
        let time_area_response = interact_with_streams_rect(
            &self.time_ranges_ui,
            &mut ctx.rec_cfg.time_ctrl,
            ui,
            &time_bg_area_rect,
            &streams_rect,
        );

        // Don't draw on top of the time ticks
        let lower_time_area_painter = ui.painter().with_clip_rect(Rect::from_x_y_ranges(
            time_fg_x_range,
            ui.min_rect().bottom()..=ui.max_rect().bottom(),
        ));

        // All the entity rows and their data density graphs:
        self.tree_ui(
            ctx,
            &time_area_response,
            &lower_time_area_painter,
            time_x_left,
            ui,
        );

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
            ctx.re_ui
                .draw_shadow_line(ui, rect, egui::Direction::LeftToRight);
        }

        // Put time-marker on top and last, so that you can always drag it
        time_marker_ui(
            &self.time_ranges_ui,
            &mut ctx.rec_cfg.time_ctrl,
            ctx.re_ui,
            ui,
            &time_area_painter,
            &timeline_rect,
        );

        self.time_ranges_ui.snap_time_control(ctx);

        // remember where to show the time for next frame:
        self.prev_col_width = self.next_col_right - ui.min_rect().left();
    }

    // All the entity rows and their data density graphs:
    fn tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        tree_max_y: f32,
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
                self.show_children(
                    ctx,
                    time_area_response,
                    time_area_painter,
                    tree_max_y,
                    &ctx.store_db.entity_db.tree,
                    ui,
                );
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn show_tree(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        tree_max_y: f32,
        last_path_part: &EntityPathPart,
        tree: &EntityTree,
        ui: &mut egui::Ui,
    ) {
        if !tree
            .prefix_times
            .has_timeline(ctx.rec_cfg.time_ctrl.timeline())
            && tree.num_timeless_messages() == 0
        {
            return; // ignore entities that have no data for the current timeline, nor any timeless data.
        }

        // The last part of the the path component
        let text = if tree.is_leaf() {
            last_path_part.to_string()
        } else {
            format!("{last_path_part}/") // show we have children with a /
        };

        let collapsing_header_id = ui.make_persistent_id(&tree.path);
        let default_open = tree.path.len() <= 1 && !tree.is_leaf();

        let item = Item::InstancePath(None, InstancePath::entity_splat(tree.path.clone()));
        let is_selected = ctx.selection().contains(&item);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let clip_rect_save = ui.clip_rect();
        let mut clip_rect = clip_rect_save;
        clip_rect.max.x = tree_max_y;
        ui.set_clip_rect(clip_rect);

        let re_ui::list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
        } = ListItem::new(ctx.re_ui, text)
            .width_allocation_mode(WidthAllocationMode::Compact)
            .selected(is_selected)
            .force_hovered(is_item_hovered)
            .show_collapsing(ui, collapsing_header_id, default_open, |_, ui| {
                self.show_children(
                    ctx,
                    time_area_response,
                    time_area_painter,
                    tree_max_y,
                    tree,
                    ui,
                );
            });

        ui.set_clip_rect(clip_rect_save);

        let response = response
            .on_hover_ui(|ui| re_data_ui::item_ui::entity_hover_card_ui(ui, ctx, &tree.path));

        item_ui::select_hovered_on_click(ctx, &response, slice::from_ref(&item));

        let is_closed = body_response.is_none();
        let response_rect = response.rect;
        self.next_col_right = self.next_col_right.max(response_rect.right());

        // From the left of the label, all the way to the rightmost of the time panel
        let full_width_rect = Rect::from_x_y_ranges(
            response_rect.left()..=ui.max_rect().right(),
            response_rect.y_range(),
        );

        let is_visible = ui.is_rect_visible(full_width_rect);

        // ----------------------------------------------

        // show the data in the time area:
        if is_visible {
            let row_rect =
                Rect::from_x_y_ranges(time_area_response.rect.x_range(), response_rect.y_range());

            highlight_timeline_row(ui, ctx, time_area_painter, &item, &row_rect);

            // show the density graph only if that item is closed
            if is_closed {
                let empty = re_data_store::TimeHistogram::default();
                let num_messages_at_time = tree
                    .prefix_times
                    .get(ctx.rec_cfg.time_ctrl.timeline())
                    .unwrap_or(&empty);

                data_density_graph::data_density_graph_ui(
                    &mut self.data_density_graph_painter,
                    ctx,
                    time_area_response,
                    time_area_painter,
                    ui,
                    tree.num_timeless_messages(),
                    num_messages_at_time,
                    row_rect,
                    &self.time_ranges_ui,
                    &item,
                );
            }
        }
    }

    fn show_children(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        tree_max_y: f32,
        tree: &EntityTree,
        ui: &mut egui::Ui,
    ) {
        for (last_component, child) in &tree.children {
            self.show_tree(
                ctx,
                time_area_response,
                time_area_painter,
                tree_max_y,
                last_component,
                child,
                ui,
            );
        }

        // If this is an entity:
        if !tree.components.is_empty() {
            let clip_rect_save = ui.clip_rect();

            for (component_name, data) in &tree.components {
                if !data.times.has_timeline(ctx.rec_cfg.time_ctrl.timeline())
                    && data.num_timeless_messages() == 0
                {
                    continue; // ignore fields that have no data for the current timeline
                }

                let component_path = ComponentPath::new(tree.path.clone(), *component_name);
                let component_name = component_path.component_name.short_name();
                let item = Item::ComponentPath(component_path);

                let mut clip_rect = clip_rect_save;
                clip_rect.max.x = tree_max_y;
                ui.set_clip_rect(clip_rect);

                let response = ListItem::new(ctx.re_ui, component_name)
                    .selected(ctx.selection().contains(&item))
                    .width_allocation_mode(WidthAllocationMode::Compact)
                    .force_hovered(
                        ctx.selection_state().highlight_for_ui_element(&item)
                            == HoverHighlight::Hovered,
                    )
                    .with_icon_fn(|_, ui, rect, visual| {
                        ui.painter()
                            .circle_filled(rect.center(), 2.0, visual.text_color());
                    })
                    .show(ui);

                ui.set_clip_rect(clip_rect_save);

                re_data_ui::item_ui::select_hovered_on_click(
                    ctx,
                    &response,
                    slice::from_ref(&item),
                );

                let response_rect = response.rect;

                self.next_col_right = self.next_col_right.max(response_rect.right());

                // From the left of the label, all the way to the rightmost of the time panel
                let full_width_rect = Rect::from_x_y_ranges(
                    response_rect.left()..=ui.max_rect().right(),
                    response_rect.y_range(),
                );
                let is_visible = ui.is_rect_visible(full_width_rect);

                if is_visible {
                    let empty_messages_over_time = TimeHistogram::default();
                    let messages_over_time = data
                        .times
                        .get(ctx.rec_cfg.time_ctrl.timeline())
                        .unwrap_or(&empty_messages_over_time);

                    // `data.times` does not contain timeless. Need to add those manually:
                    let total_num_messages =
                        messages_over_time.total_count() + data.num_timeless_messages() as u64;
                    response.on_hover_ui(|ui| {
                        ui.label(format!("Number of events: {total_num_messages}"));
                    });

                    // show the data in the time area:
                    let row_rect = Rect::from_x_y_ranges(
                        time_area_response.rect.x_range(),
                        response_rect.y_range(),
                    );

                    highlight_timeline_row(ui, ctx, time_area_painter, &item, &row_rect);

                    data_density_graph::data_density_graph_ui(
                        &mut self.data_density_graph_painter,
                        ctx,
                        time_area_response,
                        time_area_painter,
                        ui,
                        data.num_timeless_messages(),
                        messages_over_time,
                        row_rect,
                        &self.time_ranges_ui,
                        &item,
                    );
                }
            }
        }
    }

    fn top_row_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        if ui.max_rect().width() < 600.0 {
            // Responsive ui for narrow screens, e.g. mobile. Split the controls into two rows.
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let re_ui = &ctx.re_ui;
                    let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
                    let times_per_timeline = ctx.store_db.times_per_timeline();
                    self.time_control_ui
                        .play_pause_ui(time_ctrl, re_ui, times_per_timeline, ui);
                    self.time_control_ui.playback_speed_ui(time_ctrl, ui);
                    self.time_control_ui.fps_ui(time_ctrl, ui);
                });
                ui.horizontal(|ui| {
                    let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
                    self.time_control_ui.timeline_selector_ui(
                        time_ctrl,
                        ctx.store_db.times_per_timeline(),
                        ui,
                    );

                    current_time_ui(ctx, ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        help_button(ui);
                    });
                });
            });
        } else {
            // One row:
            let re_ui = &ctx.re_ui;
            let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
            let times_per_timeline = ctx.store_db.times_per_timeline();

            self.time_control_ui
                .play_pause_ui(time_ctrl, re_ui, times_per_timeline, ui);
            self.time_control_ui
                .timeline_selector_ui(time_ctrl, times_per_timeline, ui);
            self.time_control_ui.playback_speed_ui(time_ctrl, ui);
            self.time_control_ui.fps_ui(time_ctrl, ui);
            current_time_ui(ctx, ui);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                help_button(ui);
            });
        }
    }
}

/// Draw the hovered/selected highlight background for a timeline row.
fn highlight_timeline_row(
    ui: &mut Ui,
    ctx: &ViewerContext<'_>,
    painter: &Painter,
    item: &Item,
    row_rect: &Rect,
) {
    let item_hovered =
        ctx.selection_state().highlight_for_ui_element(item) == HoverHighlight::Hovered;
    let item_selected = ctx.selection().contains(item);
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

fn collapsed_time_marker_and_time(ui: &mut egui::Ui, ctx: &mut ViewerContext<'_>) {
    let space_needed_for_current_time = match ctx.rec_cfg.time_ctrl.timeline().typ() {
        re_arrow_store::TimeType::Time => 220.0,
        re_arrow_store::TimeType::Sequence => 100.0,
    };

    {
        let mut time_range_rect = ui.available_rect_before_wrap();
        time_range_rect.max.x -= space_needed_for_current_time;

        if time_range_rect.width() > 50.0 {
            let time_ranges_ui = initialize_time_ranges_ui(ctx, time_range_rect.x_range(), None);
            time_ranges_ui.snap_time_control(ctx);

            let painter = ui.painter_at(time_range_rect.expand(4.0));
            painter.hline(
                time_range_rect.x_range(),
                time_range_rect.center().y,
                ui.visuals().widgets.inactive.fg_stroke,
            );
            time_marker_ui(
                &time_ranges_ui,
                &mut ctx.rec_cfg.time_ctrl,
                ctx.re_ui,
                ui,
                &painter,
                &time_range_rect,
            );

            ui.allocate_rect(time_range_rect, egui::Sense::hover());
        }
    }

    current_time_ui(ctx, ui);
}

fn help_button(ui: &mut egui::Ui) {
    // TODO(andreas): Nicer help text like on space views.
    re_ui::help_hover_button(ui).on_hover_text(
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

/// A user can drag the time slider to between the timeless data and the first real data.
///
/// The time interpolated there is really weird, as it goes from [`TimeInt::BEGINNING`]
/// (which is extremely long time ago) to whatever tim the user logged.
/// So we do not want to display these times to the user.
///
/// This functions returns `true` iff the given time is safe to show.
fn is_time_safe_to_show(
    store_db: &re_data_store::StoreDb,
    timeline: &re_arrow_store::Timeline,
    time: TimeReal,
) -> bool {
    if store_db.num_timeless_messages() == 0 {
        return true; // no timeless messages, no problem
    }

    if let Some(times) = store_db.entity_db.tree.prefix_times.get(timeline) {
        if let Some(first_time) = times.min_key() {
            let margin = match timeline.typ() {
                re_arrow_store::TimeType::Time => TimeInt::from_seconds(10_000),
                re_arrow_store::TimeType::Sequence => TimeInt::from_sequence(1_000),
            };

            return TimeInt::from(first_time) <= time + margin;
        }
    }

    TimeInt::BEGINNING < time
}

fn current_time_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    if let Some(time_int) = ctx.rec_cfg.time_ctrl.time_int() {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        if is_time_safe_to_show(ctx.store_db, timeline, time_int.into()) {
            let time_type = ctx.rec_cfg.time_ctrl.time_type();
            ui.monospace(time_type.format(time_int));
        }
    }
}

// ----------------------------------------------------------------------------

fn initialize_time_ranges_ui(
    ctx: &mut ViewerContext<'_>,
    time_x_range: Rangef,
    mut time_view: Option<TimeView>,
) -> TimeRangesUi {
    re_tracing::profile_function!();

    // If there's any timeless data, add the "beginning range" that contains timeless data.
    let mut time_range = if ctx.store_db.num_timeless_messages() > 0 {
        vec![TimeRange {
            min: TimeInt::BEGINNING,
            max: TimeInt::BEGINNING,
        }]
    } else {
        Vec::new()
    };

    if let Some(times) = ctx
        .store_db
        .entity_db
        .tree
        .prefix_times
        .get(ctx.rec_cfg.time_ctrl.timeline())
    {
        // NOTE: `times` can be empty if a GC wiped everything.
        if !times.is_empty() {
            let timeline_axis = TimelineAxis::new(ctx.rec_cfg.time_ctrl.time_type(), times);
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
        1.0 // too narrow to fit everything anyways
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
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
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
            shadow_mesh.colored_vertex(right_pos, re_ui.design_tokens.shadow_gradient_dark_start);

            left_line_strip.push(left_pos);
            right_line_strip.push(right_pos);

            y += zig_height;
            row += 1;
        }

        // Regular & shadow mesh have the same topology!
        shadow_mesh.indices = mesh.indices.clone();

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
    ui: &mut egui::Ui,
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
            delta_x += input.scroll_delta.x;
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
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
) {
    // timeline_rect: top part with the second ticks and time marker

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let time_drag_id = ui.id().with("time_drag_id");
    let timeline_cursor_icon = CursorIcon::ResizeHorizontal;
    let is_hovering_the_loop_selection = ui.output(|o| o.cursor_icon) != CursorIcon::Default; // A kind of hacky proxy
    let is_anything_being_dragged = ui.memory(|mem| mem.is_anything_being_dragged());
    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    let mut is_hovering = false;

    // show current time as a line:
    if let Some(time) = time_ctrl.time() {
        if let Some(x) = time_ranges_ui.x_from_time_f32(time) {
            if timeline_rect.x_range().contains(x) {
                let line_rect =
                    Rect::from_x_y_ranges(x..=x, timeline_rect.top()..=ui.max_rect().bottom())
                        .expand(interact_radius);

                let response = ui
                    .interact(line_rect, time_drag_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(timeline_cursor_icon);

                is_hovering = !is_anything_being_dragged && response.hovered();

                if response.dragged() {
                    if let Some(pointer_pos) = pointer_pos {
                        if let Some(time) = time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                            let time = time_ranges_ui.clamp_time(time);
                            time_ctrl.set_time(time);
                            time_ctrl.pause();
                        }
                    }
                }

                let stroke = if response.dragged() {
                    ui.style().visuals.widgets.active.fg_stroke
                } else if is_hovering {
                    ui.style().visuals.widgets.hovered.fg_stroke
                } else {
                    ui.visuals().widgets.inactive.fg_stroke
                };
                re_ui.paint_time_cursor(
                    time_area_painter,
                    x,
                    Rangef::new(timeline_rect.top(), ui.max_rect().bottom()),
                    stroke,
                );
            }
        }
    }

    // "click here to view time here"
    if let Some(pointer_pos) = pointer_pos {
        let is_pointer_in_timeline_rect = timeline_rect.contains(pointer_pos);

        // Show preview?
        if !is_hovering
            && is_pointer_in_timeline_rect
            && !is_anything_being_dragged
            && !is_hovering_the_loop_selection
        {
            time_area_painter.vline(
                pointer_pos.x,
                timeline_rect.top()..=ui.max_rect().bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            ui.ctx().set_cursor_icon(timeline_cursor_icon); // preview!
        }

        // Click to move time here:
        if ui.input(|i| i.pointer.primary_down())
            && is_pointer_in_timeline_rect
            && !is_anything_being_dragged
            && !is_hovering_the_loop_selection
        {
            if let Some(time) = time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                let time = time_ranges_ui.clamp_time(time);
                time_ctrl.set_time(time);
                time_ctrl.pause();
                ui.memory_mut(|mem| mem.set_dragged_id(time_drag_id));
            }
        }
    }
}
