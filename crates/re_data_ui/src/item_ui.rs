//! Basic ui elements & interaction for most `re_viewer_context::Item`.
//!
//! TODO(andreas): This is not a `data_ui`, can this go somewhere else, shouldn't be in `re_data_ui`.

use re_entity_db::{EntityTree, InstancePath};
use re_log_types::{ComponentPath, EntityPath, TimeInt, Timeline};
use re_ui::SyntaxHighlighting;
use re_viewer_context::{HoverHighlight, Item, SpaceViewId, UiVerbosity, ViewerContext};

use super::DataUi;

// TODO(andreas): This is where we want to go, but we need to figure out how get the [`re_viewer_context::SpaceViewClass`] from the `SpaceViewId`.
// Simply pass in optional icons?
//
// Show a button to an [`Item`] with a given text.
// pub fn item_button_to(
//     ctx: &ViewerContext<'_>,
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
//     }
// }

/// Show an entity path and make it selectable.
pub fn entity_path_button(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        store,
        ui,
        space_view_id,
        &InstancePath::entity_splat(entity_path.clone()),
        entity_path.syntax_highlighted(ui.style()),
    )
}

/// Show the different parts of an entity path and make them selectable.
pub fn entity_path_parts_buttons(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        let mut accumulated = Vec::new();
        for part in entity_path.iter() {
            accumulated.push(part.clone());

            ui.strong("/");
            entity_path_button_to(
                ctx,
                query,
                store,
                ui,
                space_view_id,
                &accumulated.clone().into(),
                part.syntax_highlighted(ui.style()),
            );
        }
    })
    .response
}

/// Show an entity path and make it selectable.
pub fn entity_path_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        store,
        ui,
        space_view_id,
        &InstancePath::entity_splat(entity_path.clone()),
        text,
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        store,
        ui,
        space_view_id,
        instance_path,
        instance_path.syntax_highlighted(ui.style()),
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let item = Item::InstancePath(space_view_id, instance_path.clone());

    let response = ctx
        .re_ui
        .selectable_label_with_icon(
            ui,
            &re_ui::icons::ENTITY,
            text,
            ctx.selection().contains_item(&item),
            re_ui::LabelStyle::Normal,
        )
        .on_hover_ui(|ui| {
            instance_hover_card_ui(ui, ctx, query, store, instance_path);
        });

    cursor_interact_with_selectable(ctx, response, item)
}

/// Show the different parts of an instance path and make them selectable.
pub fn instance_path_parts_buttons(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        let mut accumulated = Vec::new();
        for part in instance_path.entity_path.iter() {
            accumulated.push(part.clone());

            ui.strong("/");
            entity_path_button_to(
                ctx,
                query,
                store,
                ui,
                space_view_id,
                &accumulated.clone().into(),
                part.syntax_highlighted(ui.style()),
            );
        }

        if !instance_path.instance_key.is_splat() {
            ui.strong("/");
            instance_path_button_to(
                ctx,
                query,
                store,
                ui,
                space_view_id,
                instance_path,
                instance_path.instance_key.syntax_highlighted(ui.style()),
            );
        }
    })
    .response
}

fn entity_tree_stats_ui(ui: &mut egui::Ui, timeline: &Timeline, tree: &EntityTree) {
    use re_format::format_bytes;

    // Show total bytes used in whole subtree
    let total_bytes = tree.subtree.data_bytes();

    let subtree_caveat = if tree.children.is_empty() {
        ""
    } else {
        " (including subtree)"
    };

    if total_bytes == 0 {
        return;
    }

    let mut data_rate = None;

    // Try to estimate data-rate
    if let Some(time_histogram) = tree.subtree.time_histogram.get(timeline) {
        // `num_events` is approximate - we could be logging a Tensor image and a transform
        // at _almost_ approximately the same time, but it should only count as one fence-post.
        let num_events = time_histogram.total_count(); // TODO(emilk): we should ask the histogram to count the number of non-zero keys instead.

        if let (Some(min_time), Some(max_time)) =
            (time_histogram.min_key(), time_histogram.max_key())
        {
            if min_time < max_time && 1 < num_events {
                // Let's do our best to avoid fencepost errors.
                // If we log 1 MiB once every second, then after three
                // events we have a span of 2 seconds, and 3 MiB,
                // but the data rate is still 1 MiB/s.
                //
                //          <-----2 sec----->
                // t:       0s      1s      2s
                // data:   1MiB    1MiB    1MiB

                let duration = max_time - min_time;

                let mut bytes_per_time = total_bytes as f64 / duration as f64;

                // Fencepost adjustment:
                bytes_per_time *= (num_events - 1) as f64 / num_events as f64;

                data_rate = Some(match timeline.typ() {
                    re_log_types::TimeType::Time => {
                        let bytes_per_second = 1e9 * bytes_per_time;

                        format!(
                            "{}/s in '{}'",
                            format_bytes(bytes_per_second),
                            timeline.name()
                        )
                    }

                    re_log_types::TimeType::Sequence => {
                        format!("{} / {}", format_bytes(bytes_per_time), timeline.name())
                    }
                });
            }
        }
    }

    if let Some(data_rate) = data_rate {
        ui.label(format!(
            "Using {}{subtree_caveat} ≈ {}",
            format_bytes(total_bytes as f64),
            data_rate
        ));
    } else {
        ui.label(format!(
            "Using {}{subtree_caveat}",
            format_bytes(total_bytes as f64)
        ));
    }
}

/// Show a component path and make it selectable.
pub fn component_path_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    component_path: &ComponentPath,
) -> egui::Response {
    component_path_button_to(
        ctx,
        ui,
        component_path.component_name.short_name(),
        component_path,
    )
    .on_hover_text(component_path.component_name.full_name()) // we should show the full name somewhere
}

/// Show a component path and make it selectable.
pub fn component_path_button_to(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    component_path: &ComponentPath,
) -> egui::Response {
    let item = Item::ComponentPath(component_path.clone());
    let response = ctx.re_ui.selectable_label_with_icon(
        ui,
        &re_ui::icons::COMPONENT,
        text,
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_blueprint_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
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
        .selectable_label(ctx.selection().contains_item(&item), text)
        .on_hover_ui(|ui| {
            entity_hover_card_ui(ui, ctx, query, store, entity_path);
        });
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn time_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &Timeline,
    value: TimeInt,
) -> egui::Response {
    let is_selected = ctx
        .rec_cfg
        .time_ctrl
        .read()
        .is_time_selected(timeline, value);

    let response = ui.selectable_label(
        is_selected,
        timeline.typ().format(value, ctx.app_options.time_zone),
    );
    if response.clicked() {
        ctx.rec_cfg
            .time_ctrl
            .write()
            .set_timeline_and_time(*timeline, value);
        ctx.rec_cfg.time_ctrl.write().pause();
    }
    response
}

pub fn timeline_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &Timeline,
) -> egui::Response {
    timeline_button_to(ctx, ui, timeline.name().to_string(), timeline)
}

pub fn timeline_button_to(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    timeline: &Timeline,
) -> egui::Response {
    let is_selected = ctx.rec_cfg.time_ctrl.read().timeline() == timeline;

    let response = ui
        .selectable_label(is_selected, text)
        .on_hover_text("Click to switch to this timeline");
    if response.clicked() {
        let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
        time_ctrl.set_timeline(*timeline);
        time_ctrl.pause();
    }
    response
}

// TODO(andreas): Move elsewhere, this is not directly part of the item_ui.
pub fn cursor_interact_with_selectable(
    ctx: &ViewerContext<'_>,
    response: egui::Response,
    item: Item,
) -> egui::Response {
    let is_item_hovered =
        ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

    ctx.select_hovered_on_click(&response, item);
    // TODO(andreas): How to deal with shift click for selecting ranges?

    if is_item_hovered {
        response.highlight()
    } else {
        response
    }
}

/// Displays the "hover card" (i.e. big tooltip) for an instance or an entity.
///
/// The entity hover card is displayed the provided instance path is a splat.
pub fn instance_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    instance_path: &InstancePath,
) {
    if ctx.entity_db.is_known_entity(&instance_path.entity_path) {
        ui.label("Unknown entity.");
        return;
    }

    let subtype_string = if instance_path.instance_key.is_splat() {
        "Entity"
    } else {
        "Entity Instance"
    };
    ui.strong(subtype_string);
    ui.label(instance_path.syntax_highlighted(ui.style()));

    // TODO(emilk): give data_ui an alternate "everything on this timeline" query?
    // Then we can move the size view into `data_ui`.

    if instance_path.instance_key.is_splat() {
        if let Some(subtree) = ctx.entity_db.tree().subtree(&instance_path.entity_path) {
            entity_tree_stats_ui(ui, &query.timeline, subtree);
        }
    } else {
        // TODO(emilk): per-component stats
    }

    instance_path.data_ui(ctx, ui, UiVerbosity::Reduced, query, store);
}

/// Displays the "hover card" (i.e. big tooltip) for an entity.
pub fn entity_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    store: &re_data_store::DataStore,
    entity_path: &EntityPath,
) {
    let instance_path = InstancePath::entity_splat(entity_path.clone());
    instance_hover_card_ui(ui, ctx, query, store, &instance_path);
}
