//! Basic ui elements & interaction for most `re_viewer_context::Item`.
//!
//! TODO(andreas): This is not a `data_ui`, can this go somewhere else, shouldn't be in `re_data_ui`.

use re_entity_db::{EntityTree, InstancePath};
use re_format::format_uint;
use re_log_types::{ApplicationId, ComponentPath, EntityPath, TimeInt, Timeline};
use re_ui::{icons, list_item, SyntaxHighlighting, UiExt as _};
use re_viewer_context::{HoverHighlight, Item, UiLayout, ViewId, ViewerContext};

use super::DataUi;

// TODO(andreas): This is where we want to go, but we need to figure out how get the [`re_viewer_context::ViewClass`] from the `ViewId`.
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
//         Item::View(view_id) => {
//             view_button_to(ctx, ui, text, *view_id, view_category)
//         }
//         Item::InstancePath(view_id, instance_path) => {
//             instance_path_button_to(ctx, ui, *view_id, instance_path, text)
//         }
//     }
// }

/// Show an entity path and make it selectable.
pub fn entity_path_button(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        db,
        ui,
        view_id,
        &InstancePath::entity_all(entity_path.clone()),
        entity_path.syntax_highlighted(ui.style()),
    )
}

/// Show the different parts of an entity path and make them selectable.
pub fn entity_path_parts_buttons(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
    entity_path: &EntityPath,
) -> egui::Response {
    let with_individual_icons = false; // too much noise with icons in a path

    ui.horizontal(|ui| {
        {
            ui.spacing_mut().item_spacing.x = 2.0;

            // The last part points to the selected entity, but that's ugly, so remove the highlight:
            let visuals = ui.visuals_mut();
            visuals.selection.bg_fill = egui::Color32::TRANSPARENT;
            visuals.selection.stroke = visuals.widgets.inactive.fg_stroke;
        }

        if !with_individual_icons {
            // Show one single icon up-front instead:
            let instance_path = InstancePath::entity_all(entity_path.clone());
            ui.add(instance_path_icon(&query.timeline(), db, &instance_path).as_image());
        }

        if entity_path.is_root() {
            ui.strong("/");
        } else {
            let mut accumulated = Vec::new();
            for part in entity_path.iter() {
                accumulated.push(part.clone());

                ui.strong("/");
                instance_path_button_to_ex(
                    ctx,
                    query,
                    db,
                    ui,
                    view_id,
                    &InstancePath::entity_all(accumulated.clone().into()),
                    part.syntax_highlighted(ui.style()),
                    with_individual_icons,
                );
            }
        }
    })
    .response
}

/// Show an entity path that is part of the blueprint and make it selectable.
///
/// Like [`entity_path_button_to`] but with the apriori knowledge that this exists in the blueprint.
pub fn blueprint_entity_path_button_to(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    // If we're targeting an entity in the blueprint store,
    // it doesn't make much sense to specify the view id since view ids are
    // embedded in entity paths of the blueprint store.
    // I.e. if there is a view relationship that we would care about, we would know that from the path!
    let view_id = None;

    entity_path_button_to(
        ctx,
        ctx.blueprint_query,
        ctx.blueprint_db(),
        ui,
        view_id,
        entity_path,
        text,
    )
}

/// Show an entity path and make it selectable.
pub fn entity_path_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
    entity_path: &EntityPath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        db,
        ui,
        view_id,
        &InstancePath::entity_all(entity_path.clone()),
        text,
    )
}

/// Show an instance id and make it selectable.
pub fn instance_path_button(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
    instance_path: &InstancePath,
) -> egui::Response {
    instance_path_button_to(
        ctx,
        query,
        db,
        ui,
        view_id,
        instance_path,
        instance_path.syntax_highlighted(ui.style()),
    )
}

/// Return the instance path icon.
///
/// The choice of icon is based on whether the instance is "empty" as in hasn't any logged component
/// _on the current timeline_.
pub fn instance_path_icon(
    timeline: &re_chunk_store::Timeline,
    db: &re_entity_db::EntityDb,
    instance_path: &InstancePath,
) -> &'static icons::Icon {
    if instance_path.is_all() {
        // It is an entity path
        if db
            .storage_engine()
            .store()
            .all_components_on_timeline(timeline, &instance_path.entity_path)
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
) -> (re_chunk_store::LatestAtQuery, &'a re_entity_db::EntityDb) {
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
) -> &'static icons::Icon {
    let (query, db) = guess_query_and_db_for_selected_entity(ctx, &instance_path.entity_path);
    instance_path_icon(&query.timeline(), db, instance_path)
}

/// Show an instance id and make it selectable.
pub fn instance_path_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    instance_path_button_to_ex(ctx, query, db, ui, view_id, instance_path, text, true)
}

/// Show an instance id and make it selectable.
#[allow(clippy::too_many_arguments)]
fn instance_path_button_to_ex(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
    instance_path: &InstancePath,
    text: impl Into<egui::WidgetText>,
    with_icon: bool,
) -> egui::Response {
    let item = if let Some(view_id) = view_id {
        Item::DataResult(view_id, instance_path.clone())
    } else {
        Item::InstancePath(instance_path.clone())
    };

    let response = if with_icon {
        ui.selectable_label_with_icon(
            instance_path_icon(&query.timeline(), db, instance_path),
            text,
            ctx.selection().contains_item(&item),
            re_ui::LabelStyle::Normal,
        )
    } else {
        ui.selectable_label(ctx.selection().contains_item(&item), text)
    };

    let response = response.on_hover_ui(|ui| {
        let include_subtree = false;
        instance_hover_card_ui(ui, ctx, query, db, instance_path, include_subtree);
    });

    cursor_interact_with_selectable(ctx, response, item)
}

/// Show the different parts of an instance path and make them selectable.
pub fn instance_path_parts_buttons(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    view_id: Option<ViewId>,
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
                view_id,
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
                view_id,
                instance_path,
                instance_path.instance.syntax_highlighted(ui.style()),
                with_icon,
            );
            ui.weak("]");
        }
    })
    .response
}

/// If `include_subtree=true`, stats for the entire entity subtree will be shown.
fn entity_tree_stats_ui(
    ui: &mut egui::Ui,
    timeline: &Timeline,
    db: &re_entity_db::EntityDb,
    tree: &EntityTree,
    include_subtree: bool,
) {
    use re_format::format_bytes;

    let subtree_caveat = if tree.children.is_empty() {
        ""
    } else if include_subtree {
        " (including subtree)"
    } else {
        " (excluding subtree)"
    };

    let engine = db.storage_engine();

    let (static_stats, timeline_stats) = if include_subtree {
        (
            db.subtree_stats_static(&engine, &tree.path),
            db.subtree_stats_on_timeline(&engine, &tree.path, timeline),
        )
    } else {
        (
            engine.store().entity_stats_static(&tree.path),
            engine
                .store()
                .entity_stats_on_timeline(&tree.path, timeline),
        )
    };

    let total_stats = static_stats + timeline_stats;

    if total_stats.num_rows == 0 {
        return;
    } else if timeline_stats.num_rows == 0 {
        ui.label(format!(
            "{} static rows{subtree_caveat}",
            format_uint(total_stats.num_rows)
        ));
    } else if static_stats.num_rows == 0 {
        ui.label(format!(
            "{} rows on timeline '{timeline}'{subtree_caveat}",
            format_uint(total_stats.num_rows),
            timeline = timeline.name()
        ));
    } else {
        ui.label(format!(
            "{} rows = {} static + {} on timeline '{timeline}'{subtree_caveat}",
            format_uint(total_stats.num_rows),
            format_uint(static_stats.num_rows),
            format_uint(timeline_stats.num_rows),
            timeline = timeline.name()
        ));
    }

    let num_temporal_rows = timeline_stats.num_rows;

    let mut data_rate = None;

    if 0 < timeline_stats.total_size_bytes && 1 < num_temporal_rows {
        // Try to estimate data-rate:
        if let Some(time_range) = engine.store().entity_time_range(timeline, &tree.path) {
            let min_time = time_range.min();
            let max_time = time_range.max();
            if min_time < max_time {
                // Let's do our best to avoid fencepost errors.
                // If we log 1 MiB once every second, then after three
                // events we have a span of 2 seconds, and 3 MiB,
                // but the data rate is still 1 MiB/s.
                //
                //          <-----2 sec----->
                // t:       0s      1s      2s
                // data:   1MiB    1MiB    1MiB

                let duration = max_time.as_f64() - min_time.as_f64();

                let mut bytes_per_time = timeline_stats.total_size_bytes as f64 / duration;

                // Fencepost adjustment:
                bytes_per_time *= (num_temporal_rows - 1) as f64 / num_temporal_rows as f64;

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
            "Using ~{}{subtree_caveat} ≈ {}",
            format_bytes(total_stats.total_size_bytes as f64),
            data_rate
        ));
    } else {
        ui.label(format!(
            "Using ~{}{subtree_caveat}",
            format_bytes(total_stats.total_size_bytes as f64)
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
    let is_static = db.storage_engine().store().entity_has_static_component(
        component_path.entity_path(),
        component_path.component_name(),
    );
    let icon = if is_static {
        &icons::COMPONENT_STATIC
    } else {
        &icons::COMPONENT_TEMPORAL
    };
    let response = ui.selectable_label_with_icon(
        icon,
        text,
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );

    let response = response.on_hover_ui(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend); // Make tooltip as wide as needed

        list_item::list_item_scope(ui, "component_path_tooltip", |ui| {
            ui.list_item().interactive(false).show_flat(
                ui,
                list_item::LabelContent::new(if is_static {
                    "Static component"
                } else {
                    "Temporal component"
                })
                .with_icon(icon),
            );

            component_path
                .component_name
                .data_ui_recording(ctx, ui, UiLayout::Tooltip);
        });
    });

    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_blueprint_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    view_id: ViewId,
    entity_path: &EntityPath,
) -> egui::Response {
    let item = Item::DataResult(view_id, InstancePath::entity_all(entity_path.clone()));
    let response = ui
        .selectable_label(ctx.selection().contains_item(&item), text)
        .on_hover_ui(|ui| {
            let include_subtree = false;
            entity_hover_card_ui(ui, ctx, query, db, entity_path, include_subtree);
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

    ctx.handle_select_hover_drag_interactions(&response, item, false);
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
///
/// If `include_subtree=true`, stats for the entire entity subtree will be shown.
pub fn instance_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    instance_path: &InstancePath,
    include_subtree: bool,
) {
    if !db.is_known_entity(&instance_path.entity_path) {
        ui.label("Unknown entity.");
        return;
    }

    ui.horizontal(|ui| {
        let subtype_string = if instance_path.instance.is_all() {
            "Entity"
        } else {
            "Entity instance"
        };
        ui.strong(subtype_string);
        ui.label(instance_path.syntax_highlighted(ui.style()));
    });

    // TODO(emilk): give data_ui an alternate "everything on this timeline" query?
    // Then we can move the size view into `data_ui`.

    if instance_path.instance.is_all() {
        if let Some(subtree) = db.tree().subtree(&instance_path.entity_path) {
            entity_tree_stats_ui(ui, &query.timeline(), db, subtree, include_subtree);
        }
    } else {
        // TODO(emilk): per-component stats
    }

    instance_path.data_ui(ctx, ui, UiLayout::Tooltip, query, db);
}

/// Displays the "hover card" (i.e. big tooltip) for an entity.
///
/// If `include_subtree=true`, stats for the entire entity subtree will be shown.
pub fn entity_hover_card_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    entity_path: &EntityPath,
    include_subtree: bool,
) {
    let instance_path = InstancePath::entity_all(entity_path.clone());
    instance_hover_card_ui(ui, ctx, query, db, &instance_path, include_subtree);
}

pub fn app_id_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    app_id: &ApplicationId,
) -> egui::Response {
    let item = Item::AppId(app_id.clone());

    let response = ui.selectable_label_with_icon(
        &icons::APPLICATION,
        app_id.to_string(),
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );

    let response = response.on_hover_ui(|ui| {
        app_id.data_ui_recording(ctx, ui, re_viewer_context::UiLayout::Tooltip);
    });

    cursor_interact_with_selectable(ctx, response, item)
}

pub fn data_source_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    data_source: &re_smart_channel::SmartChannelSource,
) -> egui::Response {
    let item = Item::DataSource(data_source.clone());

    let response = ui.selectable_label_with_icon(
        &icons::DATA_SOURCE,
        data_source.to_string(),
        ctx.selection().contains_item(&item),
        re_ui::LabelStyle::Normal,
    );

    let response = response.on_hover_ui(|ui| {
        data_source.data_ui_recording(ctx, ui, re_viewer_context::UiLayout::Tooltip);
    });

    cursor_interact_with_selectable(ctx, response, item)
}

/// This uses [`list_item::ListItem::show_hierarchical`], meaning it comes with built-in
/// indentation.
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
/// This uses [`list_item::ListItem::show_hierarchical`], meaning it comes with built-in
/// indentation.
pub fn entity_db_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_db: &re_entity_db::EntityDb,
    include_app_id: bool,
) {
    use re_byte_size::SizeBytes as _;
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

    let item_content = list_item::LabelContent::new(title)
        .with_icon_fn(|ui, rect, visuals| {
            // Color icon based on whether this is the active recording or not:
            let color = if ctx.store_context.is_active(&store_id) {
                visuals.fg_stroke.color
            } else {
                ui.visuals().widgets.noninteractive.fg_stroke.color
            };
            icon.as_image().tint(color).paint_at(ui, rect);
        })
        .with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::REMOVE)
                .on_hover_text(match store_id.kind {
                    re_log_types::StoreKind::Recording => {
                        "Close this recording (unsaved data will be lost)"
                    }
                    re_log_types::StoreKind::Blueprint => {
                        "Close this blueprint (unsaved data will be lost)"
                    }
                });
            if resp.clicked() {
                ctx.command_sender
                    .send_system(SystemCommand::CloseStore(store_id.clone()));
            }
            resp
        });

    let mut list_item = ui
        .list_item()
        .selected(ctx.selection().contains_item(&item));

    if ctx.hovered().contains_item(&item) {
        list_item = list_item.force_hovered(true);
    }

    let response = list_item::list_item_scope(ui, "entity db button", |ui| {
        list_item
            .show_hierarchical(ui, item_content)
            .on_hover_ui(|ui| {
                entity_db.data_ui(
                    ctx,
                    ui,
                    re_viewer_context::UiLayout::Tooltip,
                    &ctx.current_query(),
                    entity_db,
                );
            })
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
