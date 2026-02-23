use egui::RichText;
use itertools::Itertools as _;
use re_capabilities::MainThreadToken;
use re_chunk_store::UnitChunkShared;
use re_entity_db::{InstancePath, external::re_query::LatestAtResults};
use re_format::format_plural_s;
use re_log_types::ComponentPath;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::{ArchetypeName, Component as _, ComponentDescriptor, components};
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{UiExt as _, design_tokens_of_visuals, list_item};
use re_viewer_context::{HoverHighlight, Item, UiLayout, ViewerContext};

use super::DataUi;
use crate::{ArchetypeComponentMap, extra_data_ui::ExtraDataUi};

// Showing more than this takes up too much space
const MAX_COMPONENTS_IN_TOOLTIP: usize = 3;

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!();

        let Self {
            entity_path,
            instance,
        } = self;

        let components = db
            .storage_engine()
            .store()
            .all_components_on_timeline(&query.timeline(), entity_path);

        let Some(unordered_components) = components else {
            // This is fine - e.g. we're looking at `/world` and the user has only logged to `/world/car`.
            ui_layout.label(
                ui,
                format!(
                    "{self} has no own components on timeline {:?}, but its children do",
                    query.timeline()
                ),
            );
            return;
        };

        let component_descriptors: Vec<ComponentDescriptor> = {
            let storage_engine = db.storage_engine();
            let store = storage_engine.store();
            unordered_components
                .iter()
                .filter_map(|c| store.entity_component_descriptor(entity_path, *c))
                .sorted()
                .collect_vec()
        };

        let components_by_archetype = crate::sorted_component_list_by_archetype_for_ui(
            ctx.reflection(),
            component_descriptors.iter().cloned(),
        );

        let query_results = db.storage_engine().cache().latest_at(
            query,
            entity_path,
            unordered_components.iter().copied(),
        );

        let any_missing_chunks = !query_results.missing_virtual.is_empty();

        instance_path_ui(
            ctx,
            ui,
            ui_layout,
            self,
            query,
            db,
            &components_by_archetype,
            &query_results,
        );

        if instance.is_all() {
            // There are some examples where we need to combine several archetypes for a single preview.
            // For instance `VideoFrameReference` and `VideoAsset` are used together for a single preview.
            let all_components = component_descriptors
                .iter()
                .filter_map(|descr| {
                    let chunk = query_results.components.get(&descr.component)?;
                    Some((descr.clone(), chunk.clone()))
                })
                .collect_vec();

            for (descr, shared) in &all_components {
                if let Some(data) = ExtraDataUi::from_components(
                    ctx,
                    query,
                    entity_path,
                    descr,
                    shared,
                    &all_components,
                ) {
                    data.data_ui(ctx, ui, ui_layout, query, entity_path);
                }
            }
        }

        if any_missing_chunks && db.can_fetch_chunks_from_redap() {
            // TODO(RR-3670): figure out how to handle missing chunks
            ui.loading_indicator();
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn instance_path_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    instance_path: &InstancePath,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    components_by_archetype: &ArchetypeComponentMap,
    query_results: &LatestAtResults,
) {
    let num_components = components_by_archetype
        .values()
        .map(|v| v.len())
        .sum::<usize>();

    match ui_layout {
        UiLayout::List => {
            ui_layout.label(
                ui,
                format!(
                    "{} with {}",
                    format_plural_s(components_by_archetype.len(), "archetype"),
                    format_plural_s(num_components, "total component")
                ),
            );
        }
        UiLayout::Tooltip => {
            if num_components <= MAX_COMPONENTS_IN_TOOLTIP {
                component_list_ui(
                    ctx,
                    ui,
                    ui_layout,
                    instance_path,
                    query,
                    db,
                    components_by_archetype,
                    query_results,
                );
            } else {
                // Too many to show all in a tooltip.
                let showed_short_summary = try_summary_ui_for_tooltip(
                    ctx,
                    ui,
                    ui_layout,
                    instance_path,
                    query,
                    db,
                    components_by_archetype,
                    query_results,
                )
                .is_ok();

                if !showed_short_summary {
                    // Show just a very short summary:

                    ui.list_item_label(format_plural_s(num_components, "component"));

                    let archetype_count = components_by_archetype.len();
                    ui.list_item_label(format!(
                        "{}: {}",
                        format_plural_s(archetype_count, "archetype"),
                        components_by_archetype
                            .keys()
                            .map(|archetype| {
                                if let Some(archetype) = archetype {
                                    archetype.short_name()
                                } else {
                                    "<Without archetype>"
                                }
                            })
                            .join(", ")
                    ));
                }
            }
        }
        UiLayout::SelectionPanel => {
            component_list_ui(
                ctx,
                ui,
                ui_layout,
                instance_path,
                query,
                db,
                components_by_archetype,
                query_results,
            );
        }
    }
}

/// Show the value of a single instance (e.g. a point in a point cloud),
/// focusing only on the components that are different between different points.
#[expect(clippy::too_many_arguments)]
fn try_summary_ui_for_tooltip(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    instance_path: &InstancePath,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    components_by_archetype: &ArchetypeComponentMap,
    query_results: &LatestAtResults,
) -> Result<(), ()> {
    let num_components = components_by_archetype
        .values()
        .map(|v| v.len())
        .sum::<usize>();

    if instance_path.is_all() {
        return Err(()); // not an instance
    }

    // Focus on the components that have different values per instance (non-splatted components):
    let instanced_components_by_archetype: ArchetypeComponentMap = components_by_archetype
        .iter()
        .filter_map(|(archetype_name, archetype_components)| {
            let instanced_archetype_components = archetype_components
                .iter()
                .filter(|descr| {
                    query_results
                        .components
                        .get(&descr.component)
                        .is_some_and(|unit| unit.num_instances(descr.component) > 1)
                })
                .cloned()
                .collect_vec();
            if instanced_archetype_components.is_empty() {
                None
            } else {
                Some((*archetype_name, instanced_archetype_components))
            }
        })
        .collect();

    let num_instanced_components = instanced_components_by_archetype
        .values()
        .map(|v| v.len())
        .sum::<usize>();

    if MAX_COMPONENTS_IN_TOOLTIP < num_instanced_components {
        return Err(());
    }

    component_list_ui(
        ctx,
        ui,
        ui_layout,
        instance_path,
        query,
        db,
        &instanced_components_by_archetype,
        query_results,
    );

    let num_skipped = num_components - num_instanced_components;
    ui.label(format!(
        "…plus {num_skipped} more {}",
        if num_skipped == 1 {
            "component"
        } else {
            "components"
        }
    ));

    Ok(())
}

#[expect(clippy::too_many_arguments)]
fn component_list_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    instance_path: &InstancePath,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    components_by_archetype: &ArchetypeComponentMap,
    query_results: &LatestAtResults,
) {
    let InstancePath {
        entity_path,
        instance,
    } = instance_path;

    // TODO(#7026): Instances today are too poorly defined:
    // For many archetypes it makes sense to slice through all their component arrays with the same index.
    // However, there are cases when there are multiple dimensions of slicing that make sense.
    // This is most obvious for meshes & graph nodes where there are different dimensions for vertices/edges/etc.
    //
    // For graph nodes this is particularly glaring since our indicices imply nodes today and
    // unlike with meshes it's very easy to hover & select individual nodes.
    // In order to work around the GraphEdges showing up associated with random nodes, we just hide them here.
    // (this is obviously a hack and these relationships should be formalized such that they are accessible to the UI, see ticket link above)
    let mut components_by_archetype = components_by_archetype.clone();
    if !instance.is_all() {
        for components in components_by_archetype.values_mut() {
            components.retain(|component| {
                component.component_type != Some(components::GraphEdge::name())
            });
        }
    }

    re_ui::list_item::list_item_scope(
        ui,
        egui::Id::from("component list").with(entity_path),
        |ui| {
            for (archetype, archetype_components) in &components_by_archetype {
                if archetype.is_none() && components_by_archetype.len() == 1 {
                    // They are all without archetype, so we can skip the label.
                } else {
                    archetype_label_list_item_ui(ui, archetype);
                }

                let archetype_component_units: Vec<(ComponentDescriptor, UnitChunkShared)> =
                    archetype_components
                        .iter()
                        .filter_map(|descr| {
                            let unit = query_results.components.get(&descr.component)?;
                            Some((descr.clone(), unit.clone()))
                        })
                        .collect();

                let mut missing_units = false;

                for component_descr in archetype_components {
                    if let Some(unit) = query_results.components.get(&component_descr.component) {
                        component_ui(
                            ctx,
                            ui,
                            ui_layout,
                            instance_path,
                            query,
                            db,
                            &archetype_component_units,
                            component_descr,
                            unit,
                        );
                    } else {
                        missing_units = true;
                    }
                }

                if missing_units {
                    // No data found at the moment.
                    // Maybe there is no data this early on the timeline.
                    // Maybe there _were_ data, but it has been GCed.
                    // Maybe there _will be_ data, once we have loaded it.
                    let any_missing_chunks = !query_results.missing_virtual.is_empty();
                    if any_missing_chunks && db.can_fetch_chunks_from_redap() {
                        ui.loading_indicator();
                    } else {
                        ui.weak("-"); // TODO(RR-3670): figure out how to handle missing chunks
                    }
                }
            }
        },
    );
}

#[expect(clippy::too_many_arguments)]
fn component_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    instance_path: &InstancePath,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    archetype_components: &[(ComponentDescriptor, UnitChunkShared)],
    component_descr: &ComponentDescriptor,
    unit: &UnitChunkShared,
) {
    let InstancePath {
        entity_path,
        instance,
    } = instance_path;

    let interactive = ui_layout != UiLayout::Tooltip;

    let component_path = ComponentPath::new(entity_path.clone(), component_descr.component);

    let is_static = db
        .storage_engine()
        .store()
        .entity_has_static_component(entity_path, component_descr.component);
    let icon = if is_static {
        &re_ui::icons::COMPONENT_STATIC
    } else {
        &re_ui::icons::COMPONENT_TEMPORAL
    };
    let item = Item::ComponentPath(component_path);

    let mut list_item = ui.list_item().interactive(interactive);

    if interactive {
        let is_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;
        list_item = list_item.force_hovered(is_hovered);
    }

    let data = ExtraDataUi::from_components(
        ctx,
        query,
        entity_path,
        component_descr,
        unit,
        archetype_components,
    );

    let mut content =
        re_ui::list_item::PropertyContent::new(component_descr.archetype_field_name())
            .with_icon(icon)
            .value_fn(|ui, _| {
                if instance.is_all() {
                    crate::ComponentPathLatestAtResults {
                        component_path: ComponentPath::new(
                            entity_path.clone(),
                            component_descr.component,
                        ),
                        unit,
                    }
                    .data_ui(ctx, ui, UiLayout::List, query, db);
                } else {
                    ctx.component_ui_registry().component_ui(
                        ctx,
                        ui,
                        UiLayout::List,
                        query,
                        db,
                        entity_path,
                        component_descr,
                        unit,
                        instance,
                    );
                }
            });

    if let Some(data) = &data
        && ui_layout == UiLayout::SelectionPanel
    {
        content = data
            .add_inline_buttons(ctx, MainThreadToken::from_egui_ui(ui), entity_path, content)
            .with_always_show_buttons(true);
    }

    let response = list_item.show_flat(ui, content).on_hover_ui(|ui| {
        if let Some(component_type) = component_descr.component_type {
            component_type.data_ui_recording(ctx, ui, UiLayout::Tooltip);
        }

        if let Some(array) = unit.component_batch_raw(component_descr.component) {
            re_ui::list_item::list_item_scope(ui, component_descr, |ui| {
                ui.list_item_flat_noninteractive(
                    re_ui::list_item::PropertyContent::new("Data type").value_text(
                        // TODO(#11071): use re_arrow_ui to format the datatype here
                        re_arrow_util::format_data_type(array.data_type()),
                    ),
                );
            });
        }
    });

    if interactive {
        ctx.handle_select_hover_drag_interactions(&response, item, false);
    }
}

pub fn archetype_label_list_item_ui(ui: &mut egui::Ui, archetype: &Option<ArchetypeName>) {
    ui.list_item()
        .with_y_offset(1.0)
        .with_height(20.0)
        .interactive(false)
        .show_flat(
            ui,
            list_item::LabelContent::new(
                RichText::new(
                    archetype
                        .map(|a| a.short_name())
                        .unwrap_or("Without archetype"),
                )
                .size(10.0)
                .color(design_tokens_of_visuals(ui.visuals()).list_item_strong_text),
            ),
        );
}
