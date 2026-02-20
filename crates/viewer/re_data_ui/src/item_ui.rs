//! Basic ui elements & interaction for most `re_viewer_context::Item`.
//!
//! TODO(andreas): This is not a `data_ui`, can this go somewhere else, shouldn't be in `re_data_ui`.

use re_entity_db::entity_db::EntityDbClass;
use re_entity_db::{EntityTree, InstancePath};
use re_format::format_uint;
use re_log_types::{ApplicationId, EntityPath, TableId, TimeInt, TimeType, TimelineName};
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::components::{Name, Timestamp};
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{SyntaxHighlighting as _, UiExt as _, icons, list_item};
use re_viewer_context::open_url::ViewerOpenUrl;
use re_viewer_context::{
    DataResultInteractionAddress, HoverHighlight, Item, SystemCommand, SystemCommandSender as _,
    TimeControlCommand, UiLayout, ViewId, ViewerContext,
};

use super::DataUi as _;

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
                    &InstancePath::entity_all(accumulated.clone()),
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
    timeline: &TimelineName,
    db: &re_entity_db::EntityDb,
    instance_path: &InstancePath,
) -> &'static icons::Icon {
    if instance_path.is_all() {
        // It is an entity path
        if db
            .storage_engine()
            .store()
            .entity_has_data_on_timeline(timeline, &instance_path.entity_path)
        {
            if instance_path.entity_path.is_reserved() {
                &icons::ENTITY_RESERVED
            } else {
                &icons::ENTITY
            }
        } else if instance_path.entity_path.is_reserved() {
            &icons::ENTITY_RESERVED_EMPTY
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
    if ctx.app_options().inspect_blueprint_timeline
        && ctx.store_context.blueprint.is_logged_entity(entity_path)
    {
        (
            ctx.blueprint_time_ctrl.current_query(),
            ctx.store_context.blueprint,
        )
    } else {
        (ctx.time_ctrl.current_query(), ctx.recording())
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
#[expect(clippy::too_many_arguments)]
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
        Item::DataResult(DataResultInteractionAddress {
            view_id,
            instance_path: instance_path.clone(),
            visualizer: None,
        })
    } else {
        Item::InstancePath(instance_path.clone())
    };

    let response = if with_icon {
        ui.selectable_label_with_icon(
            instance_path_icon(&query.timeline(), db, instance_path),
            text,
            ctx.is_selected_or_loading(&item),
            re_ui::LabelStyle::Normal,
        )
    } else {
        ui.selectable_label(ctx.is_selected_or_loading(&item), text)
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
                &InstancePath::entity_all(accumulated.clone()),
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
    timeline: &TimelineName,
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
        ));
    } else {
        ui.label(format!(
            "{} rows = {} static + {} on timeline '{timeline}'{subtree_caveat}",
            format_uint(total_stats.num_rows),
            format_uint(static_stats.num_rows),
            format_uint(timeline_stats.num_rows),
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

                let typ = db.timeline_type(timeline);

                data_rate = Some(match typ {
                    TimeType::Sequence => {
                        format!("{} / {}", format_bytes(bytes_per_time), timeline)
                    }

                    TimeType::DurationNs | TimeType::TimestampNs => {
                        let bytes_per_second = 1e9 * bytes_per_time;

                        format!("{}/s in '{}'", format_bytes(bytes_per_second), timeline)
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

pub fn data_blueprint_button_to(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    view_id: ViewId,
    entity_path: &EntityPath,
) -> egui::Response {
    let item = Item::DataResult(DataResultInteractionAddress::from_entity_path(
        view_id,
        entity_path.clone(),
    ));
    let response = ui
        .selectable_label(ctx.is_selected_or_loading(&item), text)
        .on_hover_ui(|ui| {
            let include_subtree = false;
            entity_hover_card_ui(ui, ctx, query, db, entity_path, include_subtree);
        });
    cursor_interact_with_selectable(ctx, response, item)
}

pub fn time_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline_name: &TimelineName,
    value: TimeInt,
) -> egui::Response {
    let is_selected = ctx.time_ctrl.is_time_selected(timeline_name, value);

    let typ = ctx.recording().timeline_type(timeline_name);

    let response = ui.selectable_label(
        is_selected,
        typ.format(value, ctx.app_options().timestamp_format),
    );
    if response.clicked() {
        ctx.send_time_commands([
            TimeControlCommand::SetActiveTimeline(*timeline_name),
            TimeControlCommand::SetTime(value.into()),
            TimeControlCommand::Pause,
        ]);
    }
    response
}

pub fn timeline_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    timeline: &TimelineName,
) -> egui::Response {
    timeline_button_to(ctx, ui, timeline.to_string(), timeline)
}

pub fn timeline_button_to(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    timeline_name: &TimelineName,
) -> egui::Response {
    let is_selected = ctx.time_ctrl.timeline_name() == timeline_name;

    let response = ui
        .selectable_label(is_selected, text)
        .on_hover_text("Click to switch to this timeline");
    if response.clicked() {
        ctx.send_time_commands([
            TimeControlCommand::SetActiveTimeline(*timeline_name),
            TimeControlCommand::Pause,
        ]);
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
        ctx.is_selected_or_loading(&item),
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
    data_source: &re_log_channel::LogSource,
) -> egui::Response {
    let item = Item::DataSource(data_source.clone());

    let response = ui.selectable_label_with_icon(
        &icons::DATA_SOURCE,
        data_source.to_string(),
        ctx.is_selected_or_loading(&item),
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
    ui_layout: UiLayout,
) {
    if let Some(entity_db) = ctx.store_bundle().get(store_id) {
        entity_db_button_ui(ctx, ui, entity_db, ui_layout, true);
    } else {
        ui_layout.label(ui, "<unknown store>").on_hover_ui(|ui| {
            ui.label(format!("{store_id:?}"));
        });
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
    ui_layout: UiLayout,
    include_app_id: bool,
) -> egui::Response {
    re_tracing::profile_function!();

    use re_viewer_context::{SystemCommand, SystemCommandSender as _};

    let app_id_prefix = if include_app_id {
        format!("{} - ", entity_db.application_id())
    } else {
        String::default()
    };

    // We try to use a name that has the most chance to be familiar to the user:
    // - The recording name has to be explicitly set by the user, so use it if it exists.
    // - For remote data, segment id have a lot of visibility too, so good fall-back.
    // - Lacking anything better, the start time is better than a random id and caters to the local
    //   workflow where the same logging process is run repeatedly.
    let recording_name = if let Some(recording_name) =
        entity_db.recording_info_property::<Name>(RecordingInfo::descriptor_name().component)
    {
        Some(recording_name.to_string())
    } else if let EntityDbClass::DatasetSegment(url) = entity_db.store_class() {
        Some(url.segment_id.clone())
    } else {
        entity_db
            .recording_info_property::<Timestamp>(RecordingInfo::descriptor_start_time().component)
            .map(|started| {
                re_log_types::Timestamp::from(started.0)
                    .to_jiff_zoned(ctx.app_options().timestamp_format)
                    .strftime("%H:%M:%S")
                    .to_string()
            })
    }
    .unwrap_or_else(|| "<unknown>".to_owned());

    let size = re_format::format_bytes(entity_db.byte_size_of_physical_chunks() as _);
    let title = format!("{app_id_prefix}{recording_name} - {size}");

    let store_id = entity_db.store_id().clone();
    let item = re_viewer_context::Item::StoreId(store_id.clone());

    let icon = match entity_db.store_kind() {
        re_log_types::StoreKind::Recording => &icons::RECORDING,
        re_log_types::StoreKind::Blueprint => &icons::BLUEPRINT,
    };

    let mut item_content = list_item::LabelContent::new(title).with_icon(icon);

    if ui_layout.is_selection_panel() {
        item_content = item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::CLOSE_SMALL, "Close recording")
                .on_hover_text(match store_id.kind() {
                    re_log_types::StoreKind::Recording => {
                        "Close this recording (unsaved data will be lost)"
                    }
                    re_log_types::StoreKind::Blueprint => {
                        "Close this blueprint (unsaved data will be lost)"
                    }
                });
            if resp.clicked() {
                ctx.command_sender()
                    .send_system(SystemCommand::CloseRecordingOrTable(
                        store_id.clone().into(),
                    ));
            }
        });
    }

    let mut list_item = ui
        .list_item()
        .active(ctx.store_context.is_active(&store_id))
        .selected(ctx.is_selected_or_loading(&item));

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
    })
    .inner;

    if response.hovered() {
        ctx.selection_state().set_hovered(item.clone());
    }

    let new_entry: re_viewer_context::RecordingOrTable = store_id.clone().into();

    response.context_menu(|ui| {
        let url = ViewerOpenUrl::from_display_mode(ctx.store_hub(), &new_entry.display_mode())
            .and_then(|url| url.sharable_url(None));
        if ui
            .add_enabled(url.is_ok(), egui::Button::new("Copy link to segment"))
            .on_disabled_hover_text(if let Err(err) = url.as_ref() {
                format!("Can't copy a link to this segment: {err}")
            } else {
                "Can't copy a link to this segment".to_owned()
            })
            .clicked()
            && let Ok(url) = url
        {
            ctx.command_sender()
                .send_system(SystemCommand::CopyViewerUrl(url));
        }

        if ui.button("Copy segment name").clicked() {
            re_log::info!("Copied {recording_name:?} to clipboard");
            ui.ctx().copy_text(recording_name);
        }
    });

    if response.clicked() {
        // When we click on a recording, we directly activate it. This is safe to do because
        // it's non-destructive and recordings are immutable. Switching back is easy.
        // We don't do the same thing for blueprints as swapping them can be much more disruptive.
        // It is much less obvious how to undo a blueprint switch and what happened to your original
        // blueprint.
        // TODO(jleibs): We should still have an `Activate this Blueprint` button in the selection panel
        // for the blueprint.
        if store_id.is_recording() {
            ctx.command_sender()
                .send_system(SystemCommand::ActivateRecordingOrTable(new_entry));
        }
    }

    ctx.handle_select_hover_drag_interactions(&response, item.clone(), false);
    response
}

pub fn table_id_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    table_id: &TableId,
    ui_layout: UiLayout,
) {
    let item = re_viewer_context::Item::TableId(table_id.clone());

    let mut item_content = list_item::LabelContent::new(table_id.as_str()).with_icon(&icons::TABLE);

    if ui_layout.is_selection_panel() {
        item_content = item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::CLOSE_SMALL, "Close table")
                .on_hover_text("Close this table (all data will be lost)");
            if resp.clicked() {
                ctx.command_sender()
                    .send_system(SystemCommand::CloseRecordingOrTable(
                        table_id.clone().into(),
                    ));
            }
        });
    }

    let mut list_item = ui
        .list_item()
        .selected(ctx.is_selected_or_loading(&item))
        .active(ctx.active_table_id() == Some(table_id));

    if ctx.hovered().contains_item(&item) {
        list_item = list_item.force_hovered(true);
    }

    let response = list_item::list_item_scope(ui, "entity db button", |ui| {
        list_item
            .show_hierarchical(ui, item_content)
            .on_hover_ui(|ui| {
                ui.label(format!("Table: {table_id}"));
            })
    })
    .inner;

    if response.hovered() {
        ctx.selection_state().set_hovered(item.clone());
    }

    if response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::ActivateRecordingOrTable(
                table_id.clone().into(),
            ));
    }
    ctx.handle_select_hover_drag_interactions(&response, item, false);
}
