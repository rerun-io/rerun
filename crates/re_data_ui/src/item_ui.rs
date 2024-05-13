//! Basic ui elements & interaction for most `re_viewer_context::Item`.
//!
//! TODO(andreas): This is not a `data_ui`, can this go somewhere else, shouldn't be in `re_data_ui`.

use re_entity_db::{EntityTree, InstancePath};
use re_log_types::{ApplicationId, ComponentPath, EntityPath, TimeInt, Timeline};
use re_ui::{icons, SyntaxHighlighting};
use re_viewer_context::{HoverHighlight, Item, SpaceViewId, UiLayout, ViewerContext};

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
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        db,
        ui,
        space_view_id,
        &InstancePath::entity_all(entity_path.clone()),
        entity_path.syntax_highlighted(ui.style()),
    )
}

/// Show the different parts of an entity path and make them selectable.
pub fn entity_path_parts_buttons(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    let with_icon = false; // too much noise with icons in a path

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;

        // Show one single icon up-front instead:
        let instance_path = InstancePath::entity_all(entity_path.clone());
        ui.add(instance_path_icon(&query.timeline(), db, &instance_path).as_image());

        let mut accumulated = Vec::new();
        for part in entity_path.iter() {
            accumulated.push(part.clone());

            ui.strong("/");
            instance_path_button_to_ex(
                ctx,
                query,
                db,
                ui,
                space_view_id,
                &InstancePath::entity_all(accumulated.clone().into()),
                part.syntax_highlighted(ui.style()),
                with_icon,
            );
        }
    })
    .response
}

/// Show an entity path and make it selectable.
pub fn entity_path_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    entity_path: &EntityPath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        db,
        ui,
        space_view_id,
        &InstancePath::entity_all(entity_path.clone()),
        text,
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        db,
        ui,
        space_view_id,
        instance_path,
        instance_path.syntax_highlighted(ui.style()),
    )
}

/// Return the instance path icon.
///
/// The choice of icon is based on whether the instance is "empty" as in hasn't any logged component
/// _on the current timeline_.
pub fn instance_path_icon(
    timeline: &re_data_store::Timeline,
    db: &re_entity_db::EntityDb,
    instance_path: &InstancePath,
) -> &'static icons::Icon {
    if instance_path.is_all() {
        // It is an entity path
        if db
            .store()
            .all_components(timeline, &instance_path.entity_path)
            .is_some()
        {
            &icons::ENTITY
        } else {
            &icons::ENTITY_EMPTY
        }
    } else {
        // An instance path
        &icons::ENTITY
    }
}

/// The current time query, based on the current time control and an `entity_path`
///
/// If the user is inspecting the blueprint, and the `entity_path` is on the blueprint
/// timeline, then use the blueprint. Otherwise, use the recording.
// TODO(jleibs): Ideally this wouldn't be necessary and we could make the assessment
// directly from the entity_path.
pub fn guess_query_and_db_for_selected_entity<'a>(
    ctx: &'a ViewerContext<'_>,
    entity_path: &EntityPath,
) -> (re_data_store::LatestAtQuery, &'a re_entity_db::EntityDb) {
    if ctx.app_options.inspect_blueprint_timeline
        && ctx.store_context.blueprint.is_logged_entity(entity_path)
    {
        (
            ctx.blueprint_cfg.time_ctrl.read().current_query(),
            ctx.store_context.blueprint,
        )
    } else {
        (
            ctx.rec_cfg.time_ctrl.read().current_query(),
            ctx.recording(),
        )
    }
}

pub fn guess_instance_path_icon(
    ctx: &ViewerContext<'_>,
    instance_path: &InstancePath,
) -> &'static re_ui::icons::Icon {
    let (query, db) = guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);
    instance_path_icon(&query.timeline(), db, instance_path)
}

/// Show an instance id and make it selectable.
pub fn instance_path_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to_ex(ctx, query, db, ui, space_view_id, instance_path, text, true)
}

/// Show an instance id and make it selectable.
#[allow(clippy::too_many_arguments)]
fn instance_path_button_to_ex(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
    with_icon: bool,
) -> egui::Response {
    let item = if let Some(space_view_id) = space_view_id {
        Item::DataResult(space_view_id, instance_path.clone())
    } else {
        Item::InstancePath(instance_path.clone())
    };

    let response = if with_icon {
        ctx.re_ui.selectable_label_with_icon(
            ui,
            instance_path_icon(&query.timeline(), db, instance_path),
            text,
            ctx.selection().contains_item(&item),
            re_ui::LabelStyle::Normal,
        )
    } else {
        ui.selectable_label(ctx.selection().contains_item(&item), text)
    };

    let response = response.on_hover_ui(|ui| {
        instance_hover_card_ui(ui, ctx, query, db, instance_path);
    });

    cursor_interact_with_selectable(ctx, response, item)
}

/// Show the different parts of an instance path and make them selectable.
pub fn instance_path_parts_buttons(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    space_view_id: Option<SpaceViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    let with_icon = false; // too much noise with icons in a path

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;

        // Show one single icon up-front instead:
        ui.add(instance_path_icon(&query.timeline(), db, instance_path).as_image());

        let mut accumulated = Vec::new();
        for part in instance_path.entity_path.iter() {
            accumulated.push(part.clone());

            ui.strong("/");
            instance_path_button_to_ex(
                ctx,
                query,
                db,
                ui,
                space_view_id,
                &InstancePath::entity_all(accumulated.clone().into()),
                part.syntax_highlighted(ui.style()),
                with_icon,
            );
        }

        if !instance_path.instance.is_all() {
            ui.weak("[");
            instance_path_button_to_ex(
                ctx,
                query,
                db,
                ui,
                space_view_id,
                instance_path,
                instance_path.instance.syntax_highlighted(ui.style()),
                with_icon,
            );
            ui.weak("]");
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
    db: &re_entity_db::EntityDb,
) -> egui::Response {
    component_path_button_to(
        ctx,
        ui,
        component_path.component_name.short_name(),
        component_path,
        db,
    )
    .on_hover_text(component_path.component_name.full_name()) // we should show the full name somewhere
}

/// Show a component path and make it selectable.
pub fn component_path_button_to(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    component_path: &ComponentPath,
    db: &re_entity_db::EntityDb,
) -> egui::Response {
    let item = Item::ComponentPath(component_path.clone());
    let is_component_static = db.is_component_static(component_path).unwrap_or_default();
    let response = ctx.re_ui.selectable_label_with_icon(
        ui,
        if is_component_static {
            &icons::COMPONENT_STATIC
        } else {
            &icons::COMPONENT
        },
        text,
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_blueprint_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    space_view_id: SpaceViewId,
    entity_path: &EntityPath,
) -> egui::Response {
    let item = Item::DataResult(space_view_id, InstancePath::entity_all(entity_path.clone()));
    let response = ui
        .selectable_label(ctx.selection().contains_item(&item), text)
        .on_hover_ui(|ui| {
            entity_hover_card_ui(ui, ctx, query, db, entity_path);
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
/// The entity hover card is displayed if the provided instance path doesn't refer to a specific
/// instance.
pub fn instance_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    instance_path: &InstancePath,
) {
    if !ctx.recording().is_known_entity(&instance_path.entity_path) {
        ui.label("Unknown entity.");
        return;
    }

    let subtype_string = if instance_path.instance.is_all() {
        "Entity"
    } else {
        "Entity instance"
    };
    ui.strong(subtype_string);
    ui.label(instance_path.syntax_highlighted(ui.style()));

    // TODO(emilk): give data_ui an alternate "everything on this timeline" query?
    // Then we can move the size view into `data_ui`.

    if instance_path.instance.is_all() {
        if let Some(subtree) = ctx.recording().tree().subtree(&instance_path.entity_path) {
            entity_tree_stats_ui(ui, &query.timeline(), subtree);
        }
    } else {
        // TODO(emilk): per-component stats
    }

    instance_path.data_ui(ctx, ui, UiLayout::Tooltip, query, db);
}

/// Displays the "hover card" (i.e. big tooltip) for an entity.
pub fn entity_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    query: &re_data_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    entity_path: &EntityPath,
) {
    let instance_path = InstancePath::entity_all(entity_path.clone());
    instance_hover_card_ui(ui, ctx, query, db, &instance_path);
}

pub fn app_id_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    app_id: &ApplicationId,
) -> egui::Response {
    let item = Item::AppId(app_id.clone());

    let response = ctx.re_ui.selectable_label_with_icon(
        ui,
        &icons::APPLICATION,
        app_id.to_string(),
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );

    let response = response.on_hover_ui(|ui| {
        app_id.data_ui(
            ctx,
            ui,
            re_viewer_context::UiLayout::Tooltip,
            &ctx.current_query(), // unused
            ctx.recording(),      // unused
        );
    });

    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_source_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    data_source: &re_smart_channel::SmartChannelSource,
) -> egui::Response {
    let item = Item::DataSource(data_source.clone());

    let response = ctx.re_ui.selectable_label_with_icon(
        ui,
        &icons::DATA_SOURCE,
        data_source.to_string(),
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );

    let response = response.on_hover_ui(|ui| {
        data_source.data_ui(
            ctx,
            ui,
            re_viewer_context::UiLayout::Tooltip,
            &ctx.current_query(),
            ctx.recording(), // unused
        );
    });

    cursor_interact_with_selectable(ctx, response, item)
}

/// This uses [`re_ui::ListItem::show_hierarchical`], meaning it comes with built-in indentation.
pub fn store_id_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    store_id: &re_log_types::StoreId,
) {
    if let Some(entity_db) = ctx.store_context.bundle.get(store_id) {
        entity_db_button_ui(ctx, ui, entity_db, true);
    } else {
        ui.label(store_id.to_string());
    }
}

/// Show button for a store (recording or blueprint).
///
/// You can set `include_app_id` to hide the App Id, but usually you want to show it.
///
/// This uses [`re_ui::ListItem::show_hierarchical`], meaning it comes with built-in indentation.
pub fn entity_db_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_db: &re_entity_db::EntityDb,
    include_app_id: bool,
) {
    use re_types_core::SizeBytes as _;
    use re_viewer_context::{SystemCommand, SystemCommandSender as _};

    let app_id_prefix = if include_app_id {
        entity_db
            .app_id()
            .map_or(String::default(), |app_id| format!("{app_id} - "))
    } else {
        String::default()
    };

    let creation_time = entity_db
        .store_info()
        .and_then(|info| {
            info.started
                .format_time_custom("[hour]:[minute]:[second]", ctx.app_options.time_zone)
        })
        .unwrap_or("<unknown time>".to_owned());

    let size = re_format::format_bytes(entity_db.total_size_bytes() as _);
    let title = format!("{app_id_prefix}{creation_time} - {size}");

    let store_id = entity_db.store_id().clone();
    let item = re_viewer_context::Item::StoreId(store_id.clone());

    let icon = match entity_db.store_kind() {
        re_log_types::StoreKind::Recording => &icons::RECORDING,
        re_log_types::StoreKind::Blueprint => &icons::BLUEPRINT,
    };

    let mut list_item =
        ctx.re_ui
            .list_item(title)
            .selected(ctx.selection().contains_item(&item))
            .with_icon_fn(|_re_ui, ui, rect, visuals| {
                // Color icon based on whether this is the active recording or not:
                let color = if ctx.store_context.is_active(&store_id) {
                    visuals.fg_stroke.color
                } else {
                    ui.visuals().widgets.noninteractive.fg_stroke.color
                };
                icon.as_image().tint(color).paint_at(ui, rect);
            })
            .with_buttons(|re_ui, ui| {
                // Close-button:
                let resp = re_ui.small_icon_button(ui, &icons::REMOVE).on_hover_text(
                    match store_id.kind {
                        re_log_types::StoreKind::Recording => {
                            "Close this recording (unsaved data will be lost)"
                        }
                        re_log_types::StoreKind::Blueprint => {
                            "Close this blueprint (unsaved data will be lost)"
                        }
                    },
                );
                if resp.clicked() {
                    ctx.command_sender
                        .send_system(SystemCommand::CloseStore(store_id.clone()));
                }
                resp
            });

    if ctx.hovered().contains_item(&item) {
        list_item = list_item.force_hovered(true);
    }

    let response = list_item.show_hierarchical(ui).on_hover_ui(|ui| {
        entity_db.data_ui(
            ctx,
            ui,
            re_viewer_context::UiLayout::Tooltip,
            &ctx.current_query(),
            entity_db,
        );
    });

    if response.hovered() {
        ctx.selection_state().set_hovered(item.clone());
    }

    if response.clicked() {
        // When we click on a recording, we directly activate it. This is safe to do because
        // it's non-destructive and recordings are immutable. Switching back is easy.
        // We don't do the same thing for blueprints as swapping them can be much more disruptive.
        // It is much less obvious how to undo a blueprint switch and what happened to your original
        // blueprint.
        // TODO(jleibs): We should still have an `Activate this Blueprint` button in the selection panel
        // for the blueprint.
        if store_id.kind == re_log_types::StoreKind::Recording {
            ctx.command_sender
                .send_system(SystemCommand::ActivateRecording(store_id.clone()));
        }

        ctx.command_sender
            .send_system(SystemCommand::SetSelection(item));
    }
}
