//! Basic ui elements & interaction for most `re_viewer_context::Item`.
//!
//! TODO(andreas): This is not a `data_ui`, can this go somewhere else, shouldn't be in `re_data_ui`.

use egui::Ui;
use re_data_store::InstancePath;
use re_log_types::{ComponentPath, EntityPath, TimeInt, Timeline};
use re_viewer_context::{
    DataQueryId, HoverHighlight, Item, SpaceViewId, UiVerbosity, ViewerContext,
};

use super::DataUi;

// TODO(andreas): This is where we want to go, but we need to figure out how get the [`re_viewer_context::SpaceViewClass`] from the `SpaceViewId`.
// Simply pass in optional icons?
//
// Show a button to an [`Item`] with a given text.
// pub fn item_button_to(
//     ctx: &mut ViewerContext<'_>,
//     ui: &mut egui::Ui,
//     item: &Item,
//     text: impl Into<egui::WidgetText>,
// ) -> egui::Response {
//     match item {
//         Item::ComponentPath(component_path) => {
//             component_path_button_to(ctx, ui, text, component_path)
//         }
//         Item::SpaceView(space_view_id) => {
//             space_view_button_to(ctx, ui, text, *space_view_id, space_view_category)
//         }
//         Item::InstancePath(space_view_id, instance_path) => {
//             instance_path_button_to(ctx, ui, *space_view_id, instance_path, text)
//         }
//         Item::DataBlueprintGroup(space_view_id, group_handle) => {
//             data_blueprint_group_button_to(ctx, ui, text, *space_view_id, *group_handle)
//         }
//     }
// }

/// Show an entity path and make it selectable.
pub fn entity_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        ui,
        space_view_id,
        &InstancePath::entity_splat(entity_path.clone()),
        entity_path.to_string(),
    )
}

/// Show an entity path and make it selectable.
pub fn entity_path_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        ui,
        space_view_id,
        &InstancePath::entity_splat(entity_path.clone()),
        text,
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        ui,
        space_view_id,
        instance_path,
        instance_path.to_string(),
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let item = Item::InstancePath(space_view_id, instance_path.clone());

    let response = ui
        .selectable_label(ctx.selection().contains(&item), text)
        .on_hover_ui(|ui| {
            instance_hover_card_ui(ui, ctx, instance_path);
        });

    cursor_interact_with_selectable(ctx, response, item)
}

fn entity_stats_ui(ui: &mut egui::Ui, timeline: &Timeline, stats: &re_arrow_store::EntityStats) {
    use re_format::format_bytes;

    let total_bytes = stats.size_bytes + stats.timelines_size_bytes;

    if total_bytes == 0 {
        return;
    }

    // `num_events` is approximate - we could be logging a Tensor image and a transform
    // at approximately the same time. That should only count as one fence-post.
    let num_events = stats.num_rows;

    if stats.time_range.min < stats.time_range.max && 1 < num_events {
        // Estimate a data rate.
        //
        // Let's do our best to avoid fencepost errors.
        // If we log 1 MiB every second, then after three
        // events we have a span of 2 seconds, and 3 MiB,
        // but the data rate is still 1 MiB/s.
        //
        //          <-----2 sec----->
        // t:       0s      1s      2s
        // data:   1MiB    1MiB    1MiB

        let duration = stats.time_range.abs_length();

        let mut bytes_per_time = stats.size_bytes as f64 / duration as f64;

        // Fencepost adjustment:
        bytes_per_time *= (num_events - 1) as f64 / num_events as f64;

        let data_rate = match timeline.typ() {
            re_log_types::TimeType::Time => {
                let bytes_per_second = 1e9 * bytes_per_time;

                format!(
                    "{}/s in {}",
                    format_bytes(bytes_per_second),
                    timeline.name()
                )
            }

            re_log_types::TimeType::Sequence => {
                format!("{} / {}", format_bytes(bytes_per_time), timeline.name())
            }
        };

        ui.label(format!(
            "Using {} in total ≈ {}",
            format_bytes(total_bytes as f64),
            data_rate
        ));
    } else {
        ui.label(format!(
            "Using {} in total",
            format_bytes(total_bytes as f64)
        ));
    }
}

/// Show a component path and make it selectable.
pub fn component_path_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    component_path: &ComponentPath,
) -> egui::Response {
    component_path_button_to(
        ctx,
        ui,
        component_path.component_name.short_name(),
        component_path,
    )
}

/// Show a component path and make it selectable.
pub fn component_path_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    component_path: &ComponentPath,
) -> egui::Response {
    let item = Item::ComponentPath(component_path.clone());
    let response = ui.selectable_label(ctx.selection().contains(&item), text);
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_blueprint_group_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    space_view_id: SpaceViewId,
    query_id: DataQueryId,
    entity_path: EntityPath,
) -> egui::Response {
    let item = Item::DataBlueprintGroup(space_view_id, query_id, entity_path);
    let response = ctx
        .re_ui
        .selectable_label_with_icon(
            ui,
            &re_ui::icons::CONTAINER,
            text,
            ctx.selection().contains(&item),
        )
        .on_hover_text("Group");
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_blueprint_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    space_view_id: SpaceViewId,
    entity_path: &EntityPath,
) -> egui::Response {
    let item = Item::InstancePath(
        Some(space_view_id),
        InstancePath::entity_splat(entity_path.clone()),
    );
    let response = ui
        .selectable_label(ctx.selection().contains(&item), text)
        .on_hover_ui(|ui| {
            entity_hover_card_ui(ui, ctx, entity_path);
        });
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn time_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &Timeline,
    value: TimeInt,
) -> egui::Response {
    let is_selected = ctx.rec_cfg.time_ctrl.is_time_selected(timeline, value);

    let response = ui.selectable_label(
        is_selected,
        timeline
            .typ()
            .format(value, ctx.app_options.time_zone_for_timestamps),
    );
    if response.clicked() {
        ctx.rec_cfg
            .time_ctrl
            .set_timeline_and_time(*timeline, value);
        ctx.rec_cfg.time_ctrl.pause();
    }
    response
}

pub fn timeline_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &Timeline,
) -> egui::Response {
    timeline_button_to(ctx, ui, timeline.name().to_string(), timeline)
}

pub fn timeline_button_to(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    timeline: &Timeline,
) -> egui::Response {
    let is_selected = ctx.rec_cfg.time_ctrl.timeline() == timeline;

    let response = ui
        .selectable_label(is_selected, text)
        .on_hover_text("Click to switch to this timeline");
    if response.clicked() {
        ctx.rec_cfg.time_ctrl.set_timeline(*timeline);
        ctx.rec_cfg.time_ctrl.pause();
    }
    response
}

// TODO(andreas): Move elsewhere, this is not directly part of the item_ui.
pub fn cursor_interact_with_selectable(
    ctx: &mut ViewerContext<'_>,
    response: egui::Response,
    item: Item,
) -> egui::Response {
    let is_item_hovered =
        ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

    select_hovered_on_click(ctx, &response, &[item]);
    // TODO(andreas): How to deal with shift click for selecting ranges?

    if is_item_hovered {
        response.highlight()
    } else {
        response
    }
}

// TODO(andreas): Move elsewhere, this is not directly part of the item_ui.
pub fn select_hovered_on_click(
    ctx: &mut ViewerContext<'_>,
    response: &egui::Response,
    items: &[Item],
) {
    re_tracing::profile_function!();

    if response.hovered() {
        ctx.selection_state_mut().set_hovered(items.iter().cloned());
    }

    if response.clicked() {
        if response.ctx.input(|i| i.modifiers.command) {
            ctx.selection_state_mut().toggle_selection(items.to_vec());
        } else {
            ctx.selection_state_mut()
                .set_selection(items.iter().cloned());
        }
    }
}

/// Displays the "hover card" (i.e. big tooltip) for an instance or an entity.
///
/// The entity hover card is displayed the provided instance path is a splat.
pub fn instance_hover_card_ui(
    ui: &mut Ui,
    ctx: &mut ViewerContext<'_>,
    instance_path: &InstancePath,
) {
    let subtype_string = if instance_path.instance_key.is_splat() {
        "Entity"
    } else {
        "Entity Instance"
    };
    ui.strong(subtype_string);
    ui.label(format!("Path: {instance_path}"));

    // TODO(emilk): give data_ui an alternate "everything on this timeline" query?
    // Then we can move the size view into `data_ui`.
    let query = ctx.current_query();

    if instance_path.instance_key.is_splat() {
        let store = ctx.store_db.store();
        let stats = store.entity_stats(query.timeline, instance_path.entity_path.hash());
        entity_stats_ui(ui, &query.timeline, &stats);
    } else {
        // TODO(emilk): per-component stats
    }

    instance_path.data_ui(ctx, ui, UiVerbosity::Reduced, &query);
}

/// Displays the "hover card" (i.e. big tooltip) for an entity.
pub fn entity_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    entity_path: &EntityPath,
) {
    let instance_path = InstancePath::entity_splat(entity_path.clone());
    instance_hover_card_ui(ui, ctx, &instance_path);
}
