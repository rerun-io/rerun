mod ball_scatterer;
mod paint_ticks;
mod time_axis;
mod time_ranges_ui;
mod time_selection_ui;

use std::{collections::BTreeMap, ops::RangeInclusive};

use egui::{
    pos2, show_tooltip_at_pointer, Color32, CursorIcon, Id, NumExt, PointerButton, Rect, Shape,
    Vec2,
};
use itertools::Itertools;

use re_data_store::{EntityTree, InstancePath};
use re_log_types::{ComponentPath, EntityPathPart, TimeInt, TimeRange, TimeReal};

use crate::{Item, TimeControl, TimeView, ViewerContext};

use super::{data_ui::DataUi, selection_panel::what_is_selected_ui, Blueprint};

use time_axis::TimelineAxis;
use time_ranges_ui::TimeRangesUi;

/// A panel that shows entity names to the left, time on the top.
///
/// This includes the timeline controls and streams view.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TimePanel {
    /// Width of the entity name columns previous frame.
    prev_col_width: f32,

    /// The right side of the entity name column; updated during its painting.
    #[serde(skip)]
    next_col_right: f32,

    /// The time axis view, regenerated each frame.
    #[serde(skip)]
    time_ranges_ui: TimeRangesUi,
}

impl Default for TimePanel {
    fn default() -> Self {
        Self {
            prev_col_width: 400.0,
            next_col_right: 0.0,
            time_ranges_ui: Default::default(),
        }
    }
}

impl TimePanel {
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        ui: &mut egui::Ui,
    ) {
        let top_bar_height = 28.0;
        let margin = ctx.re_ui.bottom_panel_margin();
        let mut panel_frame = ctx.re_ui.bottom_panel_frame();

        if blueprint.time_panel_expanded {
            // Since we use scroll bars we want to fill the whole vertical space downwards:
            panel_frame.inner_margin.bottom = 0.0;

            // Similarly, let the data get close to the right edge:
            panel_frame.inner_margin.right = 0.0;
        }

        let collapsed = egui::TopBottomPanel::bottom("time_panel_collapsed")
            .resizable(false)
            .show_separator_line(false)
            .frame(panel_frame)
            .default_height(16.0);
        let expanded = egui::TopBottomPanel::bottom("time_panel_expanded")
            .resizable(true)
            .show_separator_line(false)
            .frame(panel_frame)
            .min_height(150.0)
            .default_height(250.0);

        egui::TopBottomPanel::show_animated_between_inside(
            ui,
            blueprint.time_panel_expanded,
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
                        let mut top_rop_frame = egui::Frame::default();
                        top_rop_frame.inner_margin.right = margin.x;
                        top_rop_frame.inner_margin.bottom = margin.y;
                        let rop_row_rect = top_rop_frame
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().interact_size = Vec2::splat(top_bar_height);
                                    ui.visuals_mut().button_frame = true;
                                    top_row_ui(ctx, ui);
                                });
                            })
                            .response
                            .rect;

                        // Draw separator between top bar and the rest:
                        ui.painter().hline(
                            0.0..=rop_row_rect.right(),
                            rop_row_rect.bottom(),
                            ui.visuals().widgets.noninteractive.bg_stroke,
                        );

                        ui.spacing_mut().scroll_bar_outer_margin = 4.0; // needed, because we have no panel margin on the right side.

                        // Add extra margin on the left which was intentionally missing on the controls.
                        let mut top_rop_frame = egui::Frame::default();
                        top_rop_frame.inner_margin.left = 8.0;
                        top_rop_frame.show(ui, |ui| {
                            self.expanded_ui(ctx, blueprint, ui);
                        });
                    });
                }
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn collapsed_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 18.0; // from figma

        ctx.rec_cfg
            .time_ctrl
            .time_control_ui(ctx.re_ui, ctx.log_db.times_per_timeline(), ui);

        {
            let mut time_range_rect = ui.available_rect_before_wrap();
            time_range_rect.max.x -= 220.0; // save space for current time

            if time_range_rect.width() > 50.0 {
                let time_ranges_ui =
                    initialize_time_ranges_ui(ctx, time_range_rect.x_range(), None);
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
                    ui,
                    &painter,
                    &time_range_rect,
                );

                ui.allocate_rect(time_range_rect, egui::Sense::hover());
            }
        }

        current_time_ui(ctx, ui);
    }

    fn expanded_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        ui: &mut egui::Ui,
    ) {
        crate::profile_function!();

        //               |timeline            |
        // ------------------------------------
        // tree          |streams             |
        //               |  . .   .    ...    |
        //               |             ...  . |

        self.next_col_right = ui.min_rect().left(); // next_col_right will expand during the call

        let time_x_left = ui.min_rect().left() + self.prev_col_width + ui.spacing().item_spacing.x;

        // Where the time will be shown.
        let time_bg_x_range = time_x_left..=ui.max_rect().right();
        let time_fg_x_range = {
            // Painting to the right of the scroll bar (if any) looks bad:
            let right = ui.max_rect().right() - ui.spacing_mut().scroll_bar_outer_margin;
            time_x_left..=right
        };

        let side_margin = 26.0; // chosen so that the scroll bar looks approximately centered in the default gap
        self.time_ranges_ui = initialize_time_ranges_ui(
            ctx,
            (*time_fg_x_range.start() + side_margin)..=(*time_fg_x_range.end() - side_margin),
            ctx.rec_cfg.time_ctrl.time_view(),
        );
        let full_y_range = ui.min_rect().bottom()..=ui.max_rect().bottom();

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
                "A hierarchial view of the paths used during logging.\n\
                        \n\
                        On the right you can see when there was a log event for a stream.",
            );

            let bottom = ui.min_rect().bottom();
            Rect::from_x_y_ranges(time_fg_x_range.clone(), top..=bottom)
        };

        let streams_rect = Rect::from_x_y_ranges(
            time_fg_x_range.clone(),
            timeline_rect.bottom()..=ui.max_rect().bottom(),
        );

        // includes the timeline and streams areas.
        let time_bg_area_rect = Rect::from_x_y_ranges(time_bg_x_range, full_y_range.clone());
        let time_fg_area_rect =
            Rect::from_x_y_ranges(time_fg_x_range.clone(), full_y_range.clone());
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
            full_y_range.clone(),
        );
        time_selection_ui::loop_selection_ui(
            ctx.log_db,
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

        // All the entity rows:
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            // We turn off `drag_to_scroll` so that the `ScrollArea` don't steal input from
            // the earlier `interact_with_time_area`.
            // We implement drag-to-scroll manually instead!
            .drag_to_scroll(false)
            .show(ui, |ui| {
                crate::profile_scope!("tree_ui");
                if time_area_response.dragged_by(PointerButton::Primary) {
                    ui.scroll_with_delta(Vec2::Y * time_area_response.drag_delta().y);
                }
                self.tree_ui(
                    ctx,
                    blueprint,
                    &time_area_response,
                    &lower_time_area_painter,
                    ui,
                );
            });

        {
            // Paint a shadow between the stream names on the left
            // and the data on the right:
            let shadow_width = 30.0;

            // In the design the shadow starts under the time markers.
            //let shadow_y_start =
            //    timeline_rect.bottom() + ui.visuals().widgets.noninteractive.bg_stroke.width;
            // This looks great but only if there are still time markes.
            // When they move to the right (or have a cut) one expects the shadow to go all the way up.
            // But that's quite complicated so let's have the shadow all the way
            let shadow_y_start = *full_y_range.start();

            let shadow_y_end = *full_y_range.end();
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
            ui,
            &time_area_painter,
            &timeline_rect,
        );

        self.time_ranges_ui.snap_time_control(ctx);

        // remember where to show the time for next frame:
        self.prev_col_width = self.next_col_right - ui.min_rect().left();
    }

    fn tree_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        ui: &mut egui::Ui,
    ) {
        self.show_children(
            ctx,
            blueprint,
            time_area_response,
            time_area_painter,
            &ctx.log_db.entity_db.tree,
            ui,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn show_tree(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
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
        let (_collapsing_button_response, custom_header_response, body_returned) =
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                collapsing_header_id,
                default_open,
            )
            .show_header(ui, |ui| {
                ctx.entity_path_button_to(ui, None, &tree.path, text)
            })
            .body(|ui| {
                self.show_children(
                    ctx,
                    blueprint,
                    time_area_response,
                    time_area_painter,
                    tree,
                    ui,
                );
            });

        let is_closed = body_returned.is_none();
        let response = custom_header_response.response;
        let response_rect = response.rect;
        self.next_col_right = self.next_col_right.max(response_rect.right());

        let full_width_rect = Rect::from_x_y_ranges(
            response_rect.left()..=ui.max_rect().right(),
            response_rect.y_range(),
        );

        let is_visible = ui.is_rect_visible(full_width_rect);

        if is_visible {
            // paint hline guide:
            let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            stroke.color = stroke.color.linear_multiply(0.5);
            let left = response_rect.left() + ui.spacing().indent;
            let y = response_rect.bottom() + ui.spacing().item_spacing.y * 0.5;
            ui.painter().hline(left..=ui.max_rect().right(), y, stroke);
        }

        // ----------------------------------------------

        // show the data in the time area:

        if is_visible && is_closed {
            let empty = BTreeMap::default();
            let num_messages_at_time = tree
                .prefix_times
                .get(ctx.rec_cfg.time_ctrl.timeline())
                .unwrap_or(&empty);

            show_data_over_time(
                ctx,
                blueprint,
                time_area_response,
                time_area_painter,
                ui,
                tree.num_timeless_messages(),
                num_messages_at_time,
                full_width_rect,
                &self.time_ranges_ui,
                Item::InstancePath(None, InstancePath::entity_splat(tree.path.clone())),
            );
        }
    }

    fn show_children(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        time_area_response: &egui::Response,
        time_area_painter: &egui::Painter,
        tree: &EntityTree,
        ui: &mut egui::Ui,
    ) {
        for (last_component, child) in &tree.children {
            self.show_tree(
                ctx,
                blueprint,
                time_area_response,
                time_area_painter,
                last_component,
                child,
                ui,
            );
        }

        // If this is an entity:
        if !tree.components.is_empty() {
            let indent = ui.spacing().indent;

            for (component_name, data) in &tree.components {
                if !data.times.has_timeline(ctx.rec_cfg.time_ctrl.timeline())
                    && data.num_timeless_messages() == 0
                {
                    continue; // ignore fields that have no data for the current timeline
                }

                let component_path = ComponentPath::new(tree.path.clone(), *component_name);

                let response = ui
                    .horizontal(|ui| {
                        // Add some spacing to match CollapsingHeader:
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let response =
                            ui.allocate_response(egui::vec2(indent, 0.0), egui::Sense::hover());
                        ui.painter().circle_filled(
                            response.rect.center(),
                            2.0,
                            ui.visuals().text_color(),
                        );
                        ctx.component_path_button_to(
                            ui,
                            component_name.short_name(),
                            &component_path,
                        );
                    })
                    .response;

                self.next_col_right = self.next_col_right.max(response.rect.right());

                let full_width_rect = Rect::from_x_y_ranges(
                    response.rect.left()..=ui.max_rect().right(),
                    response.rect.y_range(),
                );
                let is_visible = ui.is_rect_visible(full_width_rect);

                if is_visible {
                    // paint hline guide:
                    let mut stroke = ui.visuals().widgets.noninteractive.bg_stroke;
                    stroke.color = stroke.color.linear_multiply(0.5);
                    let left = response.rect.left() + ui.spacing().indent;
                    let y = response.rect.bottom() + ui.spacing().item_spacing.y * 0.5;
                    ui.painter().hline(left..=ui.max_rect().right(), y, stroke);
                }

                if is_visible {
                    response.on_hover_ui(|ui| {
                        let selection = Item::ComponentPath(component_path.clone());
                        what_is_selected_ui(ui, ctx, blueprint, &selection);
                        ui.add_space(8.0);
                        let query = ctx.current_query();
                        component_path.data_ui(ctx, ui, super::UiVerbosity::Small, &query);
                    });
                }

                // show the data in the time area:
                if is_visible {
                    let empty_messages_over_time = BTreeMap::default();
                    let messages_over_time = data
                        .times
                        .get(ctx.rec_cfg.time_ctrl.timeline())
                        .unwrap_or(&empty_messages_over_time);

                    show_data_over_time(
                        ctx,
                        blueprint,
                        time_area_response,
                        time_area_painter,
                        ui,
                        data.num_timeless_messages(),
                        messages_over_time,
                        full_width_rect,
                        &self.time_ranges_ui,
                        Item::ComponentPath(component_path),
                    );
                }
            }
        }
    }
}

fn top_row_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = 18.0; // from figma

    ctx.rec_cfg
        .time_ctrl
        .time_control_ui(ctx.re_ui, ctx.log_db.times_per_timeline(), ui);

    current_time_ui(ctx, ui);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        help_button(ui);
    });
}

fn help_button(ui: &mut egui::Ui) {
    crate::misc::help_hover_button(ui).on_hover_text(
        "\
        In the top row you can drag to move the time, or shift-drag to select a loop region.\n\
        \n\
        Drag main area to pan.\n\
        Zoom: Ctrl/cmd + scroll, or drag up/down with secondary mouse button.\n\
        Double-click to reset view.\n\
        \n\
        Press spacebar to play/pause.",
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
    log_db: &re_data_store::LogDb,
    timeline: &re_arrow_store::Timeline,
    time: TimeReal,
) -> bool {
    if log_db.num_timeless_messages() == 0 {
        return true; // no timeless messages, no problem
    }

    if let Some(times) = log_db.entity_db.tree.prefix_times.get(timeline) {
        if let Some(first_time) = times.keys().next() {
            let margin = match timeline.typ() {
                re_arrow_store::TimeType::Time => TimeInt::from_seconds(10_000),
                re_arrow_store::TimeType::Sequence => TimeInt::from_sequence(1_000),
            };

            return *first_time <= time + margin;
        }
    }

    TimeInt::BEGINNING < time
}

fn current_time_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    if let Some(time_int) = ctx.rec_cfg.time_ctrl.time_int() {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        if is_time_safe_to_show(ctx.log_db, timeline, time_int.into()) {
            let time_type = ctx.rec_cfg.time_ctrl.time_type();
            ui.monospace(time_type.format(time_int));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn show_data_over_time(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    time_area_response: &egui::Response,
    time_area_painter: &egui::Painter,
    ui: &mut egui::Ui,
    num_timeless_messages: usize,
    num_messages_at_time: &BTreeMap<TimeInt, usize>,
    full_width_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
    select_on_click: Item,
) {
    crate::profile_function!();

    // TODO(andreas): Should pass through underlying instance id and be clever about selection vs hover state.
    let is_selected = ctx.selection().iter().contains(&select_on_click);

    // painting each data point as a separate circle is slow (too many circles!)
    // so we join time points that are close together.
    let points_per_time = time_ranges_ui.points_per_time().unwrap_or(f64::INFINITY);
    let max_stretch_length_in_time = 1.0 / points_per_time; // TODO(emilk)

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    let hovered_color = ui.visuals().widgets.hovered.text_color();
    let inactive_color = if is_selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals()
            .widgets
            .inactive
            .text_color()
            .linear_multiply(0.75)
    };

    struct Stretch {
        start_x: f32,
        start_time: TimeInt,
        stop_time: TimeInt,
        selected: bool,
        /// Times x count at the given time
        time_points: Vec<(TimeInt, usize)>,
    }

    let mut shapes = vec![];
    let mut scatter = ball_scatterer::BallScatterer::default();
    // Time x number of messages at that time point
    let mut hovered_messages: Vec<(TimeInt, usize)> = vec![];
    let mut hovered_time = None;

    let mut paint_stretch = |stretch: &Stretch| {
        let stop_x = time_ranges_ui
            .x_from_time_f32(stretch.stop_time.into())
            .unwrap_or(stretch.start_x);

        let num_messages: usize = stretch.time_points.iter().map(|(_time, count)| count).sum();
        let radius = 2.5 * (1.0 + 0.5 * (num_messages as f32).log10());
        let radius = radius.at_most(full_width_rect.height() / 3.0);
        debug_assert!(radius.is_finite());

        let x = (stretch.start_x + stop_x) * 0.5;
        let pos = scatter.add(x, radius, (full_width_rect.top(), full_width_rect.bottom()));

        let is_hovered = pointer_pos.map_or(false, |pointer_pos| {
            pos.distance(pointer_pos) < radius + 1.0
        });

        let mut color = if is_hovered {
            hovered_color
        } else {
            inactive_color
        };
        if ui.visuals().dark_mode {
            color = color.additive();
        }

        let radius = if is_hovered {
            1.75 * radius
        } else if stretch.selected {
            1.25 * radius
        } else {
            radius
        };

        shapes.push(Shape::circle_filled(pos, radius, color));

        if is_hovered {
            hovered_messages.extend(stretch.time_points.iter().copied());
            hovered_time.get_or_insert(stretch.start_time);
        }
    };

    let selected_time_range = ctx.rec_cfg.time_ctrl.active_loop_selection();

    if num_timeless_messages > 0 {
        let time_int = TimeInt::BEGINNING;
        let time_real = TimeReal::from(time_int);
        if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
            let selected = selected_time_range.map_or(true, |range| range.contains(time_real));
            paint_stretch(&Stretch {
                start_x: x,
                start_time: time_int,
                stop_time: time_int,
                selected,
                time_points: vec![(time_int, num_timeless_messages)],
            });
        }
    }

    let mut stretch: Option<Stretch> = None;

    let margin = 5.0;
    let visible_time_range = TimeRange {
        min: time_ranges_ui
            .time_from_x_f32(time_area_painter.clip_rect().left() - margin)
            .map_or(TimeInt::MIN, |tf| tf.floor()),

        max: time_ranges_ui
            .time_from_x_f32(time_area_painter.clip_rect().right() + margin)
            .map_or(TimeInt::MAX, |tf| tf.ceil()),
    };

    for (&time, &num_messages_at_time) in
        num_messages_at_time.range(visible_time_range.min..=visible_time_range.max)
    {
        if num_messages_at_time == 0 {
            continue;
        }
        let time_real = TimeReal::from(time);

        let selected = selected_time_range.map_or(true, |range| range.contains(time_real));

        if let Some(current_stretch) = &mut stretch {
            if current_stretch.selected == selected
                && (time - current_stretch.start_time).as_f64() < max_stretch_length_in_time
            {
                // extend:
                current_stretch.stop_time = time;
                current_stretch
                    .time_points
                    .push((time, num_messages_at_time));
            } else {
                // stop the previous…
                paint_stretch(current_stretch);

                stretch = None;
            }
        }

        if stretch.is_none() {
            if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
                stretch = Some(Stretch {
                    start_x: x,
                    start_time: time,
                    stop_time: time,
                    selected,
                    time_points: vec![(time, num_messages_at_time)],
                });
            }
        }
    }

    if let Some(stretch) = stretch {
        paint_stretch(&stretch);
    }

    time_area_painter.extend(shapes);

    if !hovered_messages.is_empty() {
        if time_area_response.clicked_by(egui::PointerButton::Primary) {
            ctx.set_single_selection(select_on_click);

            if let Some(hovered_time) = hovered_time {
                ctx.rec_cfg.time_ctrl.set_time(hovered_time);
                ctx.rec_cfg.time_ctrl.pause();
            }
        } else if !ui.ctx().memory(|mem| mem.is_anything_being_dragged()) {
            show_msg_ids_tooltip(
                ctx,
                blueprint,
                ui.ctx(),
                &select_on_click,
                &hovered_messages,
            );
        }
    }
}

fn show_msg_ids_tooltip(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    egui_ctx: &egui::Context,
    item: &Item,
    time_points: &[(TimeInt, usize)],
) {
    show_tooltip_at_pointer(egui_ctx, Id::new("data_tooltip"), |ui| {
        let num_times = time_points.len();
        let num_messages: usize = time_points.iter().map(|(_time, count)| *count).sum();

        if num_times == 1 {
            if num_messages > 1 {
                ui.label(format!("{num_messages} messages"));
                ui.add_space(8.0);
                // Could be an entity made up of many components logged at the same time.
                // Still show a preview!
            }
            super::selection_panel::what_is_selected_ui(ui, ctx, blueprint, item);
            ui.add_space(8.0);

            let timeline = *ctx.rec_cfg.time_ctrl.timeline();
            let time_int = time_points[0].0; // We want to show the item at the time of whatever point we are hovering
            let query = re_arrow_store::LatestAtQuery::new(timeline, time_int);
            item.data_ui(ctx, ui, super::UiVerbosity::Reduced, &query);
        } else {
            ui.label(format!(
                "{num_messages} messages at {num_times} points in time"
            ));
        }
    });
}

// ----------------------------------------------------------------------------

fn initialize_time_ranges_ui(
    ctx: &mut ViewerContext<'_>,
    time_x_range: RangeInclusive<f32>,
    mut time_view: Option<TimeView>,
) -> TimeRangesUi {
    crate::profile_function!();

    // If there's any timeless data, add the "beginning range" that contains timeless data.
    let mut time_range = if ctx.log_db.num_timeless_messages() > 0 {
        vec![TimeRange {
            min: TimeInt::BEGINNING,
            max: TimeInt::BEGINNING,
        }]
    } else {
        Vec::new()
    };

    if let Some(times) = ctx
        .log_db
        .entity_db
        .tree
        .prefix_times
        .get(ctx.rec_cfg.time_ctrl.timeline())
    {
        let timeline_axis = TimelineAxis::new(ctx.rec_cfg.time_ctrl.time_type(), times);
        time_view = time_view.or_else(|| Some(view_everything(&time_x_range, &timeline_axis)));
        time_range.extend(timeline_axis.ranges);
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
fn view_everything(x_range: &RangeInclusive<f32>, timeline_axis: &TimelineAxis) -> TimeView {
    let gap_width = time_ranges_ui::gap_width(x_range, &timeline_axis.ranges) as f32;
    let num_gaps = timeline_axis.ranges.len().saturating_sub(1);
    let width = *x_range.end() - *x_range.start();
    let width_sans_gaps = width - num_gaps as f32 * gap_width;

    let factor = if width_sans_gaps > 0.0 {
        width / width_sans_gaps
    } else {
        1.0 // too narrow to fit everything anyways
    };

    let min = timeline_axis.min();
    let time_spanned = timeline_axis.sum_time_lengths().as_f64() * factor as f64;

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
    y_range: RangeInclusive<f32>,
) {
    crate::profile_function!();

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

    let top = *y_range.start();
    let bottom = *y_range.end();

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
            painter.vline(gap_edge, y_range.clone(), stroke);
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
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
) {
    // timeline_rect: top part with the second ticks and time marker

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    // ------------------------------------------------

    let time_drag_id = ui.id().with("time_drag_id");

    let mut is_hovering = false;

    let timeline_cursor_icon = CursorIcon::ResizeHorizontal;

    let is_hovering_the_loop_selection = ui.output(|o| o.cursor_icon) != CursorIcon::Default; // A kind of hacky proxy

    let is_anything_being_dragged = ui.memory(|mem| mem.is_anything_being_dragged());

    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    // show current time as a line:
    if let Some(time) = time_ctrl.time() {
        if let Some(x) = time_ranges_ui.x_from_time_f32(time) {
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
            paint_time_cursor(
                time_area_painter,
                x,
                timeline_rect.top()..=ui.max_rect().bottom(),
                stroke,
            );
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

pub fn paint_time_cursor(
    painter: &egui::Painter,
    x: f32,
    y: RangeInclusive<f32>,
    stroke: egui::Stroke,
) {
    let y_min = *y.start();
    let y_max = *y.end();

    let stroke = egui::Stroke {
        width: 1.5 * stroke.width,
        color: stroke.color,
    };

    let w = 10.0;
    let triangle = vec![
        pos2(x - 0.5 * w, y_min), // left top
        pos2(x + 0.5 * w, y_min), // right top
        pos2(x, y_min + w),       // bottom
    ];
    painter.add(egui::Shape::convex_polygon(
        triangle,
        stroke.color,
        egui::Stroke::NONE,
    ));
    painter.vline(x, (y_min + w)..=y_max, stroke);
}
