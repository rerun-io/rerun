use egui::{NumExt as _, Ui};

use re_data_ui::{
    image_meaning_for_entity, item_ui,
    item_ui::{guess_instance_path_icon, guess_query_and_store_for_selected_entity},
    DataUi,
};
use re_entity_db::{
    ColorMapper, Colormap, EditableAutoValue, EntityPath, EntityProperties, InstancePath,
};
use re_log_types::EntityPathFilter;
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::{
    components::{PinholeProjection, Transform3D},
    tensor_data::TensorDataMeaning,
};
use re_ui::list_item::ListItem;
use re_ui::{ReUi, SyntaxHighlighting as _};
use re_viewer_context::{
    gpu_bridge::colormap_dropdown_button_ui, ContainerId, DataQueryResult, HoverHighlight, Item,
    SpaceViewClass, SpaceViewClassIdentifier, SpaceViewId, UiVerbosity, ViewerContext,
};
use re_viewport::{
    context_menu_ui_for_item, icon_for_container_kind, space_view_name_style, Contents,
    SelectionUpdateBehavior, Viewport, ViewportBlueprint,
};

use crate::ui::override_ui::override_ui;
use crate::ui::override_ui::override_visualizer_ui;

use super::{selection_history_ui::SelectionHistoryUi, visible_history::visual_time_range_ui};

// ---

/// The "Selection View" sidebar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SelectionPanel {
    selection_state_ui: SelectionHistoryUi,
}

impl SelectionPanel {
    pub fn show_panel(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        viewport: &mut Viewport<'_, '_>,
        expanded: bool,
    ) {
        let screen_width = ui.ctx().screen_rect().width();

        let panel = egui::SidePanel::right("selection_view")
            .min_width(120.0)
            .default_width((0.45 * screen_width).min(250.0).round())
            .max_width((0.65 * screen_width).round())
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.style().visuals.panel_fill,
                ..Default::default()
            });

        // Always reset the VH highlight, and let the UI re-set it if needed.
        ctx.rec_cfg.time_ctrl.write().highlighted_range = None;

        panel.show_animated_inside(ui, expanded, |ui: &mut egui::Ui| {
            // Set the clip rectangle to the panel for the benefit of nested, "full span" widgets
            // like large collapsing headers. Here, no need to extend `ui.max_rect()` as the
            // enclosing frame doesn't have inner margins.
            ui.set_clip_rect(ui.max_rect());

            ctx.re_ui.panel_content(ui, |_, ui| {
                let hover = "The Selection View contains information and options about the \
                    currently selected object(s)";
                ctx.re_ui
                    .panel_title_bar_with_buttons(ui, "Selection", Some(hover), |ui| {
                        let mut history = ctx.selection_state().history.lock();
                        if let Some(selection) = self.selection_state_ui.selection_ui(
                            ctx.re_ui,
                            ui,
                            viewport.blueprint,
                            &mut history,
                        ) {
                            ctx.selection_state().set_selection(selection);
                        }
                    });
            });

            // move the vertical spacing between the title and the content to _inside_ the scroll
            // area
            ui.add_space(-ui.spacing().item_spacing.y);

            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(ui.spacing().item_spacing.y);
                    ctx.re_ui.panel_content(ui, |_, ui| {
                        self.contents(ctx, ui, viewport);
                    });
                });
        });
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        viewport: &mut Viewport<'_, '_>,
    ) {
        re_tracing::profile_function!();

        if ctx.selection().is_empty() {
            return;
        }

        // no gap before the first item title
        ui.add_space(-ui.spacing().item_spacing.y);

        let selection = ctx.selection();
        let multi_selection_verbosity = if selection.len() > 1 {
            UiVerbosity::LimitHeight
        } else {
            UiVerbosity::Full
        };
        for (i, item) in selection.iter_items().enumerate() {
            ui.push_id(i, |ui| {
                what_is_selected_ui(ui, ctx, viewport.blueprint, item);

                match item {
                    Item::Container(container_id) => {
                        container_top_level_properties(ui, ctx, viewport, container_id);
                        ui.add_space(12.0);
                        container_children(ui, ctx, viewport, container_id);
                    }

                    Item::SpaceView(space_view_id) => {
                        space_view_top_level_properties(ui, ctx, viewport.blueprint, space_view_id);
                    }

                    _ => {}
                }

                if let Some(data_ui_item) = data_section_ui(item) {
                    ctx.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                        let (query, store) = if let Some(entity_path) = item.entity_path() {
                            guess_query_and_store_for_selected_entity(ctx, entity_path)
                        } else {
                            (ctx.current_query(), ctx.entity_db.store())
                        };
                        data_ui_item.data_ui(ctx, ui, multi_selection_verbosity, &query, store);
                    });
                }

                // Special override section for space-view-entities
                if let Item::DataResult(space_view_id, instance_path) = item {
                    if let Some(space_view) = viewport.blueprint.space_views.get(space_view_id) {
                        // TODO(jleibs): Overrides still require special handling inside the visualizers.
                        // For now, only show the override section for TimeSeries until support is implemented
                        // generically.
                        if *space_view.class_identifier() == TimeSeriesSpaceView::identifier()
                            || ctx.app_options.experimental_visualizer_selection
                        {
                            ctx.re_ui
                                .large_collapsing_header(ui, "Visualizers", true, |ui| {
                                    override_visualizer_ui(ctx, space_view, instance_path, ui);
                                });
                            ctx.re_ui.large_collapsing_header(
                                ui,
                                "Component Overrides",
                                true,
                                |ui| {
                                    override_ui(ctx, space_view, instance_path, ui);
                                },
                            );
                        }
                    }
                }

                if has_blueprint_section(item) {
                    ctx.re_ui
                        .large_collapsing_header(ui, "Blueprint", true, |ui| {
                            blueprint_ui(ui, ctx, viewport, item);
                        });
                }

                if i < selection.len() - 1 {
                    // Add space some space between selections
                    ui.add_space(8.);
                }
            });
        }
    }
}

fn container_children(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &mut Viewport<'_, '_>,
    container_id: &ContainerId,
) {
    let Some(container) = viewport.blueprint.container(container_id) else {
        return;
    };

    ui.horizontal(|ui| {
        ui.strong("Contents");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ctx
                .re_ui
                .small_icon_button(ui, &re_ui::icons::ADD)
                .clicked()
            {
                viewport.show_add_space_view_or_container_modal(*container_id);
            }
        });
    });

    let show_content = |ui: &mut egui::Ui| {
        let mut has_child = false;
        for child_contents in &container.contents {
            has_child |= show_list_item_for_container_child(ui, ctx, viewport, child_contents);
        }

        if !has_child {
            ListItem::new(ctx.re_ui, "empty — use the + button to add content")
                .weak(true)
                .italics(true)
                .active(false)
                .show(ui);
        }
    };

    egui::Frame {
        outer_margin: egui::Margin::ZERO,
        inner_margin: egui::Margin::ZERO,
        stroke: ui.visuals().widgets.noninteractive.bg_stroke,
        ..Default::default()
    }
    .show(ui, |ui| {
        let clip_rect = ui.clip_rect();
        ui.set_clip_rect(ui.max_rect());
        ui.spacing_mut().item_spacing.y = 0.0;

        egui::Frame {
            inner_margin: egui::Margin::symmetric(4.0, 0.0),
            ..Default::default()
        }
        .show(ui, show_content);

        ui.set_clip_rect(clip_rect);
    });
}

fn data_section_ui(item: &Item) -> Option<Box<dyn DataUi>> {
    match item {
        Item::StoreId(store_id) => Some(Box::new(store_id.clone())),
        Item::ComponentPath(component_path) => Some(Box::new(component_path.clone())),
        Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
            Some(Box::new(instance_path.clone()))
        }
        // Skip data ui since we don't know yet what to show for these.
        Item::SpaceView(_) | Item::Container(_) => None,
    }
}

fn space_view_button(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view: &re_space_view::SpaceViewBlueprint,
) -> egui::Response {
    let item = Item::SpaceView(space_view.id);
    let is_selected = ctx.selection().contains_item(&item);
    let space_view_name = space_view.display_name_or_default();

    let response = ctx
        .re_ui
        .selectable_label_with_icon(
            ui,
            space_view.class(ctx.space_view_class_registry).icon(),
            space_view_name.as_ref(),
            is_selected,
            space_view_name_style(&space_view_name),
        )
        .on_hover_text("Space View");
    item_ui::cursor_interact_with_selectable(ctx, response, item)
}

/// What is selected and where is it located?
///
/// This includes a title bar and contextual information about there this item is located.
fn what_is_selected_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    item: &Item,
) {
    match item {
        Item::StoreId(store_id) => {
            let id_str = format!("{} ID: {}", store_id.kind, store_id);

            let title = if let Some(entity_db) = ctx.store_context.recording(store_id) {
                if let Some(info) = entity_db.store_info() {
                    let time = info
                        .started
                        .format_time_custom("[hour]:[minute]:[second]", ctx.app_options.time_zone)
                        .unwrap_or("<unknown time>".to_owned());

                    format!("{} - {}", info.application_id, time)
                } else {
                    id_str.clone()
                }
            } else {
                id_str.clone()
            };

            item_title_ui(ctx.re_ui, ui, &title, Some(&re_ui::icons::STORE), &id_str);
        }

        Item::Container(container_id) => {
            if let Some(container_blueprint) = viewport.container(container_id) {
                item_title_ui(
                    ctx.re_ui,
                    ui,
                    &format!("{:?}", container_blueprint.container_kind),
                    Some(re_viewport::icon_for_container_kind(
                        &container_blueprint.container_kind,
                    )),
                    &format!("{:?} container", container_blueprint.container_kind),
                );
            }
        }

        Item::ComponentPath(re_log_types::ComponentPath {
            entity_path,
            component_name,
        }) => {
            item_title_ui(
                ctx.re_ui,
                ui,
                component_name.short_name(),
                Some(&re_ui::icons::COMPONENT),
                &format!(
                    "Component {} of entity '{}'",
                    component_name.full_name(),
                    entity_path
                ),
            );

            let (query, store) = guess_query_and_store_for_selected_entity(ctx, entity_path);

            ui.horizontal(|ui| {
                ui.label("component of");
                item_ui::entity_path_button(ctx, &query, store, ui, None, entity_path);
            });

            list_existing_data_blueprints(ui, ctx, &entity_path.clone().into(), viewport);
        }

        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = viewport.space_view(space_view_id) {
                let space_view_class = space_view.class(ctx.space_view_class_registry);

                let hover_text = if let Some(display_name) = space_view.display_name.as_ref() {
                    format!(
                        "Space View {:?} of type {}",
                        display_name,
                        space_view_class.display_name()
                    )
                } else {
                    format!(
                        "Unnamed Space View of type {}",
                        space_view_class.display_name()
                    )
                };

                let space_view_name = space_view.display_name_or_default();
                ListItem::new(ctx.re_ui, space_view_name.as_ref())
                    .label_style(space_view_name_style(&space_view_name))
                    .with_icon(space_view.class(ctx.space_view_class_registry).icon())
                    .with_height(ReUi::title_bar_height())
                    .selected(true)
                    .show(ui)
                    .on_hover_text(hover_text);
            }
        }

        Item::InstancePath(instance_path) => {
            let typ = item.kind();
            let name = instance_path.syntax_highlighted(ui.style());

            item_title_ui(
                ctx.re_ui,
                ui,
                name,
                Some(guess_instance_path_icon(ctx, instance_path)),
                &format!("{typ} '{instance_path}'"),
            );

            let is_instance = !instance_path.instance_key.is_splat();
            let parent = if is_instance {
                Some(instance_path.entity_path.clone())
            } else {
                instance_path.entity_path.parent()
            };
            if let Some(parent) = parent {
                if !parent.is_root() {
                    let (query, store) =
                        guess_query_and_store_for_selected_entity(ctx, &instance_path.entity_path);
                    ui.horizontal(|ui| {
                        ui.label("Parent");
                        item_ui::entity_path_parts_buttons(ctx, &query, store, ui, None, &parent);
                    });
                }
            }

            list_existing_data_blueprints(ui, ctx, instance_path, viewport);
        }

        Item::DataResult(space_view_id, instance_path) => {
            let name = instance_path.syntax_highlighted(ui.style());

            if let Some(space_view) = viewport.space_view(space_view_id) {
                let typ = item.kind();
                item_title_ui(
                    ctx.re_ui,
                    ui,
                    name,
                    Some(guess_instance_path_icon(ctx, instance_path)),
                    &format!(
                        "{typ} '{instance_path}' as shown in Space View {:?}",
                        space_view.display_name
                    ),
                );

                let is_instance = !instance_path.instance_key.is_splat();
                let parent = if is_instance {
                    Some(instance_path.entity_path.clone())
                } else {
                    instance_path.entity_path.parent()
                };
                if let Some(parent) = parent {
                    if !parent.is_root() {
                        ui.horizontal(|ui| {
                            let (query, store) = guess_query_and_store_for_selected_entity(
                                ctx,
                                &instance_path.entity_path,
                            );

                            ui.label("Parent");
                            item_ui::entity_path_parts_buttons(
                                ctx,
                                &query,
                                store,
                                ui,
                                Some(*space_view_id),
                                &parent,
                            );
                        });
                    }
                }

                ui.horizontal(|ui| {
                    ui.label("in");
                    space_view_button(ctx, ui, space_view);
                });
            }
        }
    }
}

/// A title bar for an item.
fn item_title_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    name: impl Into<egui::WidgetText>,
    icon: Option<&re_ui::Icon>,
    hover: &str,
) -> egui::Response {
    let mut list_item = ListItem::new(re_ui, name)
        .with_height(ReUi::title_bar_height())
        .selected(true);

    if let Some(icon) = icon {
        list_item = list_item.with_icon(icon);
    }

    list_item.show(ui).on_hover_text(hover)
}

/// Display a list of all the space views an entity appears in.
fn list_existing_data_blueprints(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    instance_path: &InstancePath,
    blueprint: &ViewportBlueprint,
) {
    let space_views_with_path =
        blueprint.space_views_containing_entity_path(ctx, &instance_path.entity_path);

    let (query, store) = guess_query_and_store_for_selected_entity(ctx, &instance_path.entity_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any Space View)");
    } else {
        for space_view_id in &space_views_with_path {
            if let Some(space_view) = blueprint.space_view(space_view_id) {
                ui.horizontal(|ui| {
                    item_ui::instance_path_button_to(
                        ctx,
                        &query,
                        store,
                        ui,
                        Some(*space_view_id),
                        instance_path,
                        "Shown",
                    );
                    ui.label("in");
                    space_view_button(ctx, ui, space_view);
                });
            }
        }
    }
}

/// Display the top-level properties of a space view.
///
/// This includes the name, space origin entity, and space view type. These properties are singled
/// out as needing to be edited in most case when creating a new Space View, which is why they are
/// shown at the very top.
fn space_view_top_level_properties(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    space_view_id: &SpaceViewId,
) {
    if let Some(space_view) = viewport.space_view(space_view_id) {
        egui::Grid::new("space_view_top_level_properties")
            .num_columns(2)
            .show(ui, |ui| {
                let mut name = space_view.display_name.clone().unwrap_or_default();
                ui.label("Name").on_hover_text(
                    "The name of the Space View used for display purposes. This can be any text \
                    string.",
                );
                ui.text_edit_singleline(&mut name);
                space_view.set_display_name(ctx, if name.is_empty() { None } else { Some(name) });

                ui.end_row();

                ui.label("Space origin").on_hover_text(
                    "The origin Entity for this Space View. For spatial Space Views, the Space \
                    View's origin is the same as this Entity's origin and all transforms are \
                    relative to it.",
                );

                super::space_view_space_origin_ui::space_view_space_origin_widget_ui(
                    ui, ctx, space_view,
                );

                ui.end_row();

                ui.label("Type")
                    .on_hover_text("The type of this Space View");
                ui.label(
                    space_view
                        .class(ctx.space_view_class_registry)
                        .display_name(),
                );

                ui.end_row();
            });
    }
}

fn container_top_level_properties(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &Viewport<'_, '_>,
    container_id: &ContainerId,
) {
    let Some(container) = viewport.blueprint.container(container_id) else {
        return;
    };

    egui::Grid::new("container_top_level_properties")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Kind");

            let mut container_kind = container.container_kind;
            egui::ComboBox::from_id_source("container_kind")
                .selected_text(format!("{container_kind:?}"))
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(64.0);

                    ui.selectable_value(
                        &mut container_kind,
                        egui_tiles::ContainerKind::Tabs,
                        format!("{:?}", egui_tiles::ContainerKind::Tabs),
                    );
                    ui.selectable_value(
                        &mut container_kind,
                        egui_tiles::ContainerKind::Horizontal,
                        format!("{:?}", egui_tiles::ContainerKind::Horizontal),
                    );
                    ui.selectable_value(
                        &mut container_kind,
                        egui_tiles::ContainerKind::Vertical,
                        format!("{:?}", egui_tiles::ContainerKind::Vertical),
                    );
                    ui.selectable_value(
                        &mut container_kind,
                        egui_tiles::ContainerKind::Grid,
                        format!("{:?}", egui_tiles::ContainerKind::Grid),
                    );
                });

            viewport
                .blueprint
                .set_container_kind(*container_id, container_kind);

            ui.end_row();

            if container.container_kind == egui_tiles::ContainerKind::Grid {
                ui.label("Columns");

                fn columns_to_string(columns: &Option<u32>) -> String {
                    match columns {
                        None => "Auto".to_owned(),
                        Some(cols) => cols.to_string(),
                    }
                }

                let mut new_columns = container.grid_columns;

                egui::ComboBox::from_id_source("container_grid_columns")
                    .selected_text(columns_to_string(&new_columns))
                    .show_ui(ui, |ui| {
                        ui.style_mut().wrap = Some(false);
                        ui.set_min_width(64.0);

                        ui.selectable_value(&mut new_columns, None, columns_to_string(&None));

                        ui.separator();

                        for columns in 1..=container.contents.len() as u32 {
                            ui.selectable_value(
                                &mut new_columns,
                                Some(columns),
                                columns_to_string(&Some(columns)),
                            );
                        }
                    });

                container.set_grid_columns(ctx, new_columns);

                ui.end_row();
            }

            if ui
                .button("Simplify hierarchy")
                .on_hover_text("Simplify this container and its children")
                .clicked()
            {
                viewport.blueprint.simplify_container(
                    container_id,
                    egui_tiles::SimplificationOptions {
                        prune_empty_tabs: true,
                        prune_empty_containers: true,
                        prune_single_child_tabs: false,
                        prune_single_child_containers: false,
                        all_panes_must_have_tabs: true,
                        join_nested_linear_containers: true,
                    },
                );
            }
        });
}

// TODO(#4560): this code should be generic and part of re_data_ui
/// Show a list item for a single container child.
///
/// Return true if successful.
fn show_list_item_for_container_child(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &Viewport<'_, '_>,
    child_contents: &Contents,
) -> bool {
    let mut remove_contents = false;
    let (item, mut list_item) = match child_contents {
        Contents::SpaceView(space_view_id) => {
            let Some(space_view) = viewport.blueprint.space_view(space_view_id) else {
                re_log::warn_once!("Could not find space view with ID {space_view_id:?}",);
                return false;
            };

            let space_view_name = space_view.display_name_or_default();
            (
                Item::SpaceView(*space_view_id),
                ListItem::new(ctx.re_ui, space_view_name.as_ref())
                    .label_style(space_view_name_style(&space_view_name))
                    .with_icon(space_view.class(ctx.space_view_class_registry).icon())
                    .with_buttons(|re_ui, ui| {
                        let response = re_ui
                            .small_icon_button(ui, &re_ui::icons::REMOVE)
                            .on_hover_text("Remove this Space View");

                        if response.clicked() {
                            remove_contents = true;
                        }

                        response
                    }),
            )
        }
        Contents::Container(container_id) => {
            let Some(container) = viewport.blueprint.container(container_id) else {
                re_log::warn_once!("Could not find container with ID {container_id:?}",);
                return false;
            };

            (
                Item::Container(*container_id),
                ListItem::new(ctx.re_ui, format!("{:?}", container.container_kind))
                    .label_style(re_ui::LabelStyle::Unnamed)
                    .with_icon(icon_for_container_kind(&container.container_kind))
                    .with_buttons(|re_ui, ui| {
                        let response = re_ui
                            .small_icon_button(ui, &re_ui::icons::REMOVE)
                            .on_hover_text("Remove this Container");

                        if response.clicked() {
                            remove_contents = true;
                        }

                        response
                    }),
            )
        }
    };

    let is_item_hovered =
        ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

    if is_item_hovered {
        list_item = list_item.force_hovered(true);
    }

    let response = list_item.show(ui);

    context_menu_ui_for_item(
        ctx,
        viewport.blueprint,
        &item,
        &response,
        SelectionUpdateBehavior::Ignore,
    );
    ctx.select_hovered_on_click(&response, std::iter::once(item));

    if remove_contents {
        viewport.blueprint.mark_user_interaction(ctx);
        viewport.blueprint.remove_contents(*child_contents);
    }

    true
}

fn has_blueprint_section(item: &Item) -> bool {
    match item {
        Item::StoreId(_) | Item::ComponentPath(_) | Item::Container(_) | Item::InstancePath(_) => {
            false
        }
        Item::SpaceView(_) | Item::DataResult(_, _) => true,
    }
}

/// What is the blueprint stuff for this item?
fn blueprint_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &mut Viewport<'_, '_>,
    item: &Item,
) {
    match item {
        Item::SpaceView(space_view_id) => {
            blueprint_ui_for_space_view(ui, ctx, viewport, space_view_id);
        }

        Item::DataResult(space_view_id, instance_path) => {
            blueprint_ui_for_data_result(ui, ctx, viewport, space_view_id, instance_path);
        }

        Item::StoreId(_) | Item::ComponentPath(_) | Item::Container(_) | Item::InstancePath(_) => {}
    }
}

fn blueprint_ui_for_space_view(
    ui: &mut Ui,
    ctx: &ViewerContext<'_>,
    viewport: &mut Viewport<'_, '_>,
    space_view_id: &SpaceViewId,
) {
    if let Some(space_view) = viewport.blueprint.space_view(space_view_id) {
        if let Some(new_entity_path_filter) = entity_path_filter_ui(
            ui,
            viewport,
            space_view_id,
            &space_view.contents.entity_path_filter,
        ) {
            space_view
                .contents
                .set_entity_path_filter(ctx, &new_entity_path_filter);
        }

        ui.add_space(ui.spacing().item_spacing.y);
    }

    if ui
        .button("Clone Space View")
        .on_hover_text(
            "Create an exact duplicate of this Space View including all Blueprint settings",
        )
        .clicked()
    {
        if let Some(new_space_view_id) = viewport.blueprint.duplicate_space_view(space_view_id, ctx)
        {
            ctx.selection_state()
                .set_selection(Item::SpaceView(new_space_view_id));
            viewport.blueprint.mark_user_interaction(ctx);
        }
    }

    ui.add_space(ui.spacing().item_spacing.y / 2.0);
    ReUi::full_span_separator(ui);
    ui.add_space(ui.spacing().item_spacing.y / 2.0);

    if let Some(space_view) = viewport.blueprint.space_view(space_view_id) {
        let class_identifier = *space_view.class_identifier();

        let space_view_state = viewport.state.space_view_state_mut(
            ctx.space_view_class_registry,
            space_view.id,
            &class_identifier,
        );

        // Space View don't inherit properties.
        let space_view_data_result =
            space_view.space_view_data_result(ctx.store_context, ctx.blueprint_query);

        visual_time_range_ui(
            ctx,
            ui,
            None, // There is no tree above the space view yet
            &space_view_data_result,
            class_identifier,
            true,
        );

        let mut props = space_view_data_result
            .individual_properties()
            .cloned()
            .unwrap_or_default();

        let space_view_class = space_view.class(ctx.space_view_class_registry);
        if let Err(err) = space_view_class.selection_ui(
            ctx,
            ui,
            space_view_state.space_view_state.as_mut(),
            &space_view.space_origin,
            space_view.id,
            &mut props,
        ) {
            re_log::error!(
                "Error in Space View selection UI (class: {}, display name: {}): {err}",
                space_view.class_identifier(),
                space_view_class.display_name(),
            );
        }

        space_view_data_result.save_individual_override_properties(ctx, Some(props));
    }
}

fn blueprint_ui_for_data_result(
    ui: &mut Ui,
    ctx: &ViewerContext<'_>,
    viewport: &Viewport<'_, '_>,
    space_view_id: &SpaceViewId,
    instance_path: &InstancePath,
) {
    if let Some(space_view) = viewport.blueprint.space_view(space_view_id) {
        if instance_path.instance_key.is_splat() {
            // splat - the whole entity
            let space_view_class = *space_view.class_identifier();
            let entity_path = &instance_path.entity_path;

            let query_result = ctx.lookup_query_result(space_view.id);
            if let Some(data_result) = query_result
                .tree
                .lookup_result_by_path(entity_path)
                .cloned()
            {
                let mut props = data_result
                    .individual_properties()
                    .cloned()
                    .unwrap_or_default();

                entity_props_ui(
                    ctx,
                    ui,
                    ctx.lookup_query_result(*space_view_id),
                    &space_view_class,
                    entity_path,
                    &mut props,
                );
                data_result.save_individual_override_properties(ctx, Some(props));
            }
        }
    }
}

/// Returns a new filter when the editing is done, and there has been a change.
fn entity_path_filter_ui(
    ui: &mut egui::Ui,
    viewport: &mut Viewport<'_, '_>,
    space_view_id: &SpaceViewId,
    filter: &EntityPathFilter,
) -> Option<EntityPathFilter> {
    fn entity_path_filter_help_ui(ui: &mut egui::Ui) {
        let markdown = r#"
# Entity path query syntax

Entity path queries are described as a list of include/exclude rules that act on paths:

```diff
+ /world/**           # add everything…
- /world/roads/**     # …but remove all roads…
+ /world/roads/main   # …but show main road
```

If there are multiple matching rules, the most specific rule wins.
If there are multiple rules of the same specificity, the last one wins.
If no rules match, the path is excluded.

The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
(`/world/**` matches both `/world` and `/world/car/driver`).
Other uses of `*` are not (yet) supported.

`EntityPathFilter` sorts the rule by entity path, with recursive coming before non-recursive.
This means the last matching rule is also the most specific one.
For instance:

```diff
+ /world/**
- /world
- /world/car/**
+ /world/car/driver
```

The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it is included.
The last rule matching `/world/car/hood` is `- /world/car/**`, so it is excluded.
The last rule matching `/world` is `- /world`, so it is excluded.
The last rule matching `/world/house` is `+ /world/**`, so it is included.
    "#
        .trim();

        re_ui::markdown_ui(ui, egui::Id::new("entity_path_filter_help_ui"), markdown);
    }

    fn syntax_highlight_entity_path_filter(
        style: &egui::Style,
        mut string: &str,
    ) -> egui::text::LayoutJob {
        let font_id = egui::TextStyle::Body.resolve(style);

        let mut job = egui::text::LayoutJob::default();

        while !string.is_empty() {
            let newline = string.find('\n').unwrap_or(string.len() - 1);
            let line = &string[..=newline];
            string = &string[newline + 1..];
            let is_exclusion = line.trim_start().starts_with('-');

            let color = if is_exclusion {
                egui::Color32::LIGHT_RED
            } else {
                egui::Color32::LIGHT_GREEN
            };

            let text_format = egui::TextFormat {
                font_id: font_id.clone(),
                color,
                ..Default::default()
            };

            job.append(line, 0.0, text_format);
        }

        job
    }

    fn text_layouter(ui: &egui::Ui, string: &str, wrap_width: f32) -> std::sync::Arc<egui::Galley> {
        let mut layout_job = syntax_highlight_entity_path_filter(ui.style(), string);
        layout_job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(layout_job))
    }

    // We store the string we are temporarily editing in the `Ui`'s temporary data storage.
    // This is so it can contain invalid rules while the user edits it, and it's only normalized
    // when they press enter, or stops editing.
    let filter_text_id = ui.id().with("filter_text");

    let mut filter_string = ui.data_mut(|data| {
        data.get_temp_mut_or_insert_with::<String>(filter_text_id, || filter.formatted())
            .clone()
    });

    let rightmost_x = ui.cursor().min.x;
    ui.horizontal(|ui| {
        ui.label("Entity path query").on_hover_text(
            "The entity path query consists of a list of include/exclude rules \
            that determines what entities are part of this space view",
        );

        let current_x = ui.cursor().min.x;
        // Compute a width that results in these things to be right-aligned with the following text edit.
        let desired_width = (ui.available_width() - ui.spacing().item_spacing.x)
            .at_most(ui.spacing().text_edit_width - (current_x - rightmost_x));

        ui.allocate_ui_with_layout(
            egui::vec2(desired_width, ui.available_height()),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                re_ui::help_hover_button(ui).on_hover_ui(entity_path_filter_help_ui);
                if ui
                    .button("Edit")
                    .on_hover_text("Modify the entity query using the editor")
                    .clicked()
                {
                    viewport.show_add_remove_entities_modal(*space_view_id);
                }
            },
        );
    });

    let response =
        ui.add(egui::TextEdit::multiline(&mut filter_string).layouter(&mut text_layouter));

    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp::<String>(filter_text_id, filter_string.clone()));
    } else {
        // Reconstruct it from the filter next frame
        ui.data_mut(|data| data.remove::<String>(filter_text_id));
    }

    // Apply the edit.
    let new_filter = EntityPathFilter::parse_forgiving(&filter_string, &Default::default());
    if &new_filter == filter {
        None // no change
    } else {
        Some(new_filter)
    }
}

fn entity_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_result: &DataQueryResult,
    space_view_class: &SpaceViewClassIdentifier,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    use re_types::blueprint::components::Visible;
    use re_types::Loggable as _;

    let re_ui = ctx.re_ui;
    let Some(data_result) = query_result.tree.lookup_result_by_path(entity_path) else {
        return;
    };

    {
        let visible_before = data_result.lookup_override_or_default::<Visible>(ctx);
        let mut visible = visible_before;

        let override_source =
            data_result.component_override_source(&query_result.tree, &Visible::name());
        let is_inherited =
            override_source.is_some() && override_source.as_ref() != Some(entity_path);

        ui.horizontal(|ui| {
            re_ui.checkbox(ui, &mut visible.0, "Visible");
            if is_inherited {
                ui.label("(inherited)");
            }
        });

        if visible_before != visible {
            data_result.save_recursive_override_or_clear_if_redundant(
                ctx,
                &query_result.tree,
                &visible,
            );
        }
    }

    re_ui
        .checkbox(ui, &mut entity_props.interactive, "Interactive")
        .on_hover_text("If disabled, the entity will not react to any mouse interaction");

    visual_time_range_ui(
        ctx,
        ui,
        Some(&query_result.tree),
        data_result,
        *space_view_class,
        false,
    );

    egui::Grid::new("entity_properties")
        .num_columns(2)
        .show(ui, |ui| {
            // TODO(wumpf): It would be nice to only show pinhole & depth properties in the context of a 3D view.
            // if *view_state.state_spatial.nav_mode.get() == SpatialNavigationMode::ThreeD {
            pinhole_props_ui(ctx, ui, entity_path, entity_props);
            depth_props_ui(ctx, ui, entity_path, entity_props);
            transform3d_visualization_ui(ctx, ui, entity_path, entity_props);
        });
}

fn colormap_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_props: &mut EntityProperties,
) {
    let mut re_renderer_colormap = match *entity_props.color_mapper.get() {
        ColorMapper::Colormap(Colormap::Grayscale) => re_renderer::Colormap::Grayscale,
        ColorMapper::Colormap(Colormap::Turbo) => re_renderer::Colormap::Turbo,
        ColorMapper::Colormap(Colormap::Viridis) => re_renderer::Colormap::Viridis,
        ColorMapper::Colormap(Colormap::Plasma) => re_renderer::Colormap::Plasma,
        ColorMapper::Colormap(Colormap::Magma) => re_renderer::Colormap::Magma,
        ColorMapper::Colormap(Colormap::Inferno) => re_renderer::Colormap::Inferno,
    };

    ui.label("Color map");
    colormap_dropdown_button_ui(ctx.render_ctx, ui, &mut re_renderer_colormap);

    let new_colormap = match re_renderer_colormap {
        re_renderer::Colormap::Grayscale => Colormap::Grayscale,
        re_renderer::Colormap::Turbo => Colormap::Turbo,
        re_renderer::Colormap::Viridis => Colormap::Viridis,
        re_renderer::Colormap::Plasma => Colormap::Plasma,
        re_renderer::Colormap::Magma => Colormap::Magma,
        re_renderer::Colormap::Inferno => Colormap::Inferno,
    };
    entity_props.color_mapper = EditableAutoValue::UserEdited(ColorMapper::Colormap(new_colormap));

    ui.end_row();
}

fn pinhole_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    let (query, store) = guess_query_and_store_for_selected_entity(ctx, entity_path);
    if store
        .query_latest_component::<PinholeProjection>(entity_path, &query)
        .is_some()
    {
        ui.label("Image plane distance");
        let mut distance = *entity_props.pinhole_image_plane_distance;
        let speed = (distance * 0.05).at_least(0.01);
        if ui
            .add(
                egui::DragValue::new(&mut distance)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text("Controls how far away the image plane is")
            .changed()
        {
            entity_props.pinhole_image_plane_distance = EditableAutoValue::UserEdited(distance);
        }
        ui.end_row();
    }
}

fn depth_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) -> Option<()> {
    re_tracing::profile_function!();

    let (query, store) = guess_query_and_store_for_selected_entity(ctx, entity_path);

    let meaning = image_meaning_for_entity(entity_path, &query, store);

    if meaning != TensorDataMeaning::Depth {
        return Some(());
    }
    let image_projection_ent_path = store
        .query_latest_component_at_closest_ancestor::<PinholeProjection>(entity_path, &query)?
        .0;

    let mut backproject_depth = *entity_props.backproject_depth;

    if ctx
        .re_ui
        .checkbox(ui, &mut backproject_depth, "Backproject Depth")
        .on_hover_text(
            "If enabled, the depth texture will be backprojected into a point cloud rather \
                than simply displayed as an image.",
        )
        .changed()
    {
        entity_props.backproject_depth = EditableAutoValue::UserEdited(backproject_depth);
    }
    ui.end_row();

    if backproject_depth {
        ui.label("Pinhole");
        item_ui::entity_path_button(ctx, &query, store, ui, None, &image_projection_ent_path)
            .on_hover_text(
                "The entity path of the pinhole transform being used to do the backprojection.",
            );
        ui.end_row();

        depth_from_world_scale_ui(ui, &mut entity_props.depth_from_world_scale);

        backproject_radius_scale_ui(ui, &mut entity_props.backproject_radius_scale);

        // TODO(cmc): This should apply to the depth map entity as a whole, but for that we
        // need to get the current hardcoded colormapping out of the image cache first.
        colormap_props_ui(ctx, ui, entity_props);
    }

    Some(())
}

fn depth_from_world_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject meter");
    let mut value = *property.get();
    let speed = (value * 0.05).at_least(0.01);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .clamp_range(0.0..=1.0e8)
                .speed(speed),
        )
        .on_hover_text("How many steps in the depth image correspond to one world-space unit. For instance, 1000 means millimeters.\n\
                    Double-click to reset.");
    if response.double_clicked() {
        // reset to auto - the exact value will be restored somewhere else
        *property = EditableAutoValue::Auto(value);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}

fn backproject_radius_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject radius scale");
    let mut value = *property.get();
    let speed = (value * 0.01).at_least(0.001);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .clamp_range(0.0..=1.0e8)
                .speed(speed),
        )
        .on_hover_text(
            "Scales the radii of the points in the backprojected point cloud.\n\
            This is a factor of the projected pixel diameter. \
            This means a scale of 0.5 will leave adjacent pixels at the same depth value just touching.\n\
            Double-click to reset.",
        );
    if response.double_clicked() {
        *property = EditableAutoValue::Auto(2.0);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}

fn transform3d_visualization_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    re_tracing::profile_function!();

    let (query, store) = guess_query_and_store_for_selected_entity(ctx, entity_path);

    if store
        .query_latest_component::<Transform3D>(entity_path, &query)
        .is_none()
    {
        return;
    }

    let show_arrows = &mut entity_props.transform_3d_visible;
    let arrow_length = &mut entity_props.transform_3d_size;

    {
        let mut checked = *show_arrows.get();
        let response = ctx.re_ui.checkbox(ui, &mut checked, "Show transform").on_hover_text(
            "Enables/disables the display of three arrows to visualize the (accumulated) transform at this entity. Red/green/blue show the x/y/z axis respectively.");
        if response.changed() {
            *show_arrows = EditableAutoValue::UserEdited(checked);
        }
        if response.double_clicked() {
            *show_arrows = EditableAutoValue::Auto(checked);
        }
    }

    if *show_arrows.get() {
        ui.end_row();
        ui.label("Transform-arrow length");
        let mut value = *arrow_length.get();
        let speed = (value * 0.05).at_least(0.001);
        let response = ui
            .add(
                egui::DragValue::new(&mut value)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text(
                "How long the arrows should be in the entity's own coordinate system. Double-click to reset to auto.",
            );
        if response.double_clicked() {
            // reset to auto - the exact value will be restored somewhere else
            *arrow_length = EditableAutoValue::Auto(value);
            response.surrender_focus();
        } else if response.changed() {
            *arrow_length = EditableAutoValue::UserEdited(value);
        }
    }

    ui.end_row();
}
