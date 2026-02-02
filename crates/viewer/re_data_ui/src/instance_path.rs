use std::collections::BTreeMap;

use egui::RichText;
use itertools::Itertools as _;
use re_capabilities::MainThreadToken;
use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::{ArchetypeName, Component as _, ComponentDescriptor, components};
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{UiExt as _, design_tokens_of_visuals, list_item};
use re_viewer_context::{HoverHighlight, Item, UiLayout, ViewerContext};

use super::DataUi;
use crate::extra_data_ui::ExtraDataUi;

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

        // NOTE: the passed in `db` is usually the recording; NOT the blueprint.
        let component = if ctx.recording().is_known_entity(entity_path) {
            // We are looking at an entity in the recording
            ctx.recording_engine()
                .store()
                .all_components_on_timeline(&query.timeline(), entity_path)
        } else if ctx.blueprint_db().is_known_entity(entity_path) {
            // We are looking at an entity in the blueprint
            ctx.blueprint_db()
                .storage_engine()
                .store()
                .all_components_on_timeline(&query.timeline(), entity_path)
        } else {
            ui.error_label(format!("Unknown entity: {entity_path:?}"));
            return;
        };
        let Some(unordered_components) = component else {
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

        let components_by_archetype = {
            let recording_engine = ctx.recording_engine();
            let store = recording_engine.store();
            crate::sorted_component_list_by_archetype_for_ui(
                ctx.reflection(),
                unordered_components
                    .iter()
                    .filter_map(|c| store.entity_component_descriptor(entity_path, *c)),
            )
        };

        let mut query_results = db.storage_engine().cache().latest_at(
            query,
            entity_path,
            unordered_components.iter().copied(),
        );

        // Keep previously established order.
        let mut components_by_archetype: BTreeMap<
            Option<ArchetypeName>,
            Vec<(ComponentDescriptor, UnitChunkShared)>,
        > = components_by_archetype
            .into_iter()
            .map(|(archetype, components)| {
                (
                    archetype,
                    components
                        .into_iter()
                        .filter_map(|c| {
                            query_results
                                .components
                                .remove(&c.component)
                                .map(|chunk| (c, chunk))
                        })
                        .collect(),
                )
            })
            .collect();

        if components_by_archetype.is_empty() {
            let typ = db.timeline_type(&query.timeline());
            ui_layout.label(
                ui,
                format!(
                    "Nothing logged at {} = {}",
                    query.timeline(),
                    typ.format(query.at(), ctx.app_options().timestamp_format),
                ),
            );
            return;
        }

        // Showing more than this takes up too much space
        const MAX_COMPONENTS_IN_TOOLTIP: usize = 3;

        if ui_layout.is_single_line() {
            let archetype_count = components_by_archetype.len();
            let component_count = unordered_components.len();
            ui_layout.label(
                ui,
                format!(
                    "{} archetype{} with {} total component{}",
                    archetype_count,
                    if archetype_count > 1 { "s" } else { "" },
                    component_count,
                    if component_count > 1 { "s" } else { "" }
                ),
            );
        } else if ui_layout == UiLayout::Tooltip
            && MAX_COMPONENTS_IN_TOOLTIP < unordered_components.len()
        {
            // Too many to show all in a tooltip.

            let mut show_only_instanced = false;

            if !self.is_all() {
                // Focus on the components that have different values per instance (non-splatted components):
                let instanced_components_by_archetype: BTreeMap<
                    Option<ArchetypeName>,
                    Vec<(ComponentDescriptor, UnitChunkShared)>,
                > = components_by_archetype
                    .iter()
                    .filter_map(|(archetype_name, archetype_components)| {
                        let instanced_archetype_components = archetype_components
                            .iter()
                            .filter(|(descr, unit)| unit.num_instances(descr.component) > 1)
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

                show_only_instanced = num_instanced_components <= MAX_COMPONENTS_IN_TOOLTIP;

                if show_only_instanced {
                    component_list_ui(
                        ctx,
                        ui,
                        ui_layout,
                        query,
                        db,
                        entity_path,
                        instance,
                        &instanced_components_by_archetype,
                    );

                    let num_skipped = unordered_components.len() - num_instanced_components;
                    ui.label(format!(
                        "…plus {num_skipped} more {}",
                        if num_skipped == 1 {
                            "component"
                        } else {
                            "components"
                        }
                    ));
                }
            }

            if !show_only_instanced {
                // Show just a rough summary:

                let component_count = unordered_components.len();
                ui.list_item_label(format!(
                    "{} component{}",
                    component_count,
                    if component_count > 1 { "s" } else { "" }
                ));

                let archetype_count = components_by_archetype.len();
                ui.list_item_label(format!(
                    "{} archetype{}: {}",
                    archetype_count,
                    if archetype_count > 1 { "s" } else { "" },
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
        } else {
            // TODO(#7026): Instances today are too poorly defined:
            // For many archetypes it makes sense to slice through all their component arrays with the same index.
            // However, there are cases when there are multiple dimensions of slicing that make sense.
            // This is most obvious for meshes & graph nodes where there are different dimensions for vertices/edges/etc.
            //
            // For graph nodes this is particularly glaring since our indicices imply nodes today and
            // unlike with meshes it's very easy to hover & select individual nodes.
            // In order to work around the GraphEdges showing up associated with random nodes, we just hide them here.
            // (this is obviously a hack and these relationships should be formalized such that they are accessible to the UI, see ticket link above)
            if !self.is_all() {
                for components in components_by_archetype.values_mut() {
                    components.retain(|(component, _chunk)| {
                        component.component_type != Some(components::GraphEdge::name())
                    });
                }
            }

            component_list_ui(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                instance,
                &components_by_archetype,
            );
        }

        if instance.is_all() {
            // There are some examples where we need to combine several archetypes for a single preview.
            // For instance `VideoFrameReference` and `VideoAsset` are used together for a single preview.
            let all_components = components_by_archetype
                .values()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();

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
    }
}

#[expect(clippy::too_many_arguments)]
fn component_list_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    entity_path: &re_log_types::EntityPath,
    instance: &re_log_types::Instance,
    components_by_archetype: &BTreeMap<
        Option<ArchetypeName>,
        Vec<(ComponentDescriptor, UnitChunkShared)>,
    >,
) {
    re_ui::list_item::list_item_scope(
        ui,
        egui::Id::from("component list").with(entity_path),
        |ui| {
            for (archetype, archetype_components) in components_by_archetype {
                if archetype.is_none() && components_by_archetype.len() == 1 {
                    // They are all without archetype, so we can skip the label.
                } else {
                    archetype_label_list_item_ui(ui, archetype);
                }

                for (component_descr, unit) in archetype_components {
                    component_ui(
                        ctx,
                        ui,
                        ui_layout,
                        query,
                        db,
                        entity_path,
                        instance,
                        archetype_components,
                        component_descr,
                        unit,
                    );
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
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    entity_path: &re_log_types::EntityPath,
    instance: &re_log_types::Instance,
    archetype_components: &[(ComponentDescriptor, UnitChunkShared)],
    component_descr: &ComponentDescriptor,
    unit: &UnitChunkShared,
) {
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
