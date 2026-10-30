use itertools::Itertools as _;

use re_chunk_store::ChunkTrackingMode;
use re_data_ui::ComponentUiOptions;
use re_entity_db::InstancePath;
use re_query::LatestAllComponentResults;
use re_sdk_types::blueprint::components::ViewDensity;
use re_sdk_types::{ComponentDescriptor, ComponentIdentifier};
use re_ui::{UiExt as _, list_item};
use re_viewer_context::{StoreViewContext, UiLayout};

/// Render the configured components, grouped by archetype.
pub fn component_list_ui(
    ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    instance_path: &InstancePath,
    components: Option<&[ComponentIdentifier]>,
    style: ViewDensity,
) {
    re_tracing::profile_function!();

    // Spacious rows sit under archetype headings; compact rows name their archetype instead.
    let spacious = style == ViewDensity::Spacious;

    let engine = ctx.db.storage_engine();
    let store = engine.store();
    let entity_path = &instance_path.entity_path;
    let Some(available) = store.all_components_on_timeline(Some(&ctx.timeline_name()), entity_path)
    else {
        return;
    };

    let by_archetype = re_data_ui::sorted_component_list_by_archetype_for_ui(
        ctx.app_ctx.reflection,
        available.iter().filter_map(|&component| {
            store
                .schema()
                .entity_component_descriptor(entity_path, component)
        }),
    );
    let rows: Vec<&ComponentDescriptor> = by_archetype
        .values()
        .flatten()
        .filter(|descr| components.is_none_or(|components| components.contains(&descr.component)))
        .collect();
    let query_results = engine.cache().latest_all(
        ChunkTrackingMode::ReportTransient,
        &ctx.query(),
        entity_path,
        available.iter().copied(),
    );
    let loading = !query_results.missing_virtual.is_empty() && ctx.db.can_fetch_chunks_from_redap();
    let sole_row = rows.len() == 1;
    let show_unnamed_heading = rows.iter().any(|row| row.archetype.is_some());

    let density = match style {
        ViewDensity::Spacious => re_ui::ListItemDensity::Spacious,
        ViewDensity::Compact => re_ui::ListItemDensity::Compact,
    };
    let list_style = list_item::ListItemStyle::default().with_density(density);
    list_item::list_item_scope_with_style(ui, ("component list", entity_path), list_style, |ui| {
        let mut first_group = true;
        for (archetype, rows) in &rows.iter().chunk_by(|row| row.archetype) {
            if spacious && (archetype.is_some() || show_unnamed_heading) {
                if !first_group {
                    ui.add_space(4.0);
                }
                first_group = false;

                // Not using `re_data_ui::archetype_label_list_item_ui(ui, archetype.as_ref());`
                // Debatable, but it has a bit of a different style that doesn't quite fit in.
                ui.list_item()
                    .with_y_offset(1.0)
                    .with_density(re_ui::ListItemDensity::Compact)
                    .interactive(false)
                    .show_hierarchical(
                        ui,
                        list_item::LabelContent::new(
                            egui::RichText::new(
                                archetype
                                    .as_ref()
                                    .map_or("Without archetype", |a| a.short_name()),
                            )
                            .text_style(re_ui::DesignTokens::small())
                            .color(ui.tokens().text_subdued),
                        ),
                    );
            }

            let mut missing = false;
            for row in rows {
                if let Some(hits) = query_results.components.get(&row.component) {
                    let siblings = by_archetype.get(&archetype).map_or(&[][..], Vec::as_slice);
                    re_data_ui::component_ui(
                        ctx,
                        ui,
                        UiLayout::SelectionPanel,
                        instance_path,
                        siblings,
                        row,
                        hits,
                        &query_results,
                        ComponentUiOptions {
                            expandable: has_inner_structure(row.component, hits),
                            default_open: sole_row,
                            show_archetype: !spacious,
                            hide_type_icon: true,
                            value_ui: Some(&|ui, layout| {
                                component_value_ui(ctx, ui, layout, row.component, hits);
                            }),
                        },
                    );
                } else {
                    missing = true;
                }
            }

            if missing && !loading {
                ui.list_item().interactive(false).show_flat(
                    ui,
                    list_item::PropertyContent::new("-").with_icon_fn(|_ui, _rect, _visuals| {}),
                );
            }
        }
    });
    if loading {
        ui.loading_indicator("Fetching chunks from redap");
    }
}

fn component_value_ui(
    ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    component: ComponentIdentifier,
    hits: &LatestAllComponentResults,
) {
    let timestamp_format = ctx.app_ctx.app_options.timestamp_format;

    if ui_layout.is_single_line() {
        match hits
            .latest_row()
            .and_then(|unit| unit.component_batch_raw(component))
        {
            Some(array) => re_arrow_ui::arrow_ui(ui, ui_layout, timestamp_format, array.as_ref()),
            None => {
                ui.weak("-");
            }
        }
        return;
    }

    let mut any = false;
    for (unit_index, array) in hits
        .iter_units()
        .filter_map(|unit| unit.component_batch_raw(component))
        .enumerate()
    {
        any = true;
        let options = re_arrow_ui::DisplayOptions {
            timestamp_format,
            ..Default::default()
        };
        match re_arrow_ui::ArrayUi::try_new(array.as_ref(), &options) {
            Ok(array_ui) => {
                ui.list_item_scope(("arrow_ui", unit_index), |ui| {
                    if array.len() == 1 {
                        array_ui.show_value(0, ui);
                    } else {
                        array_ui.show(ui);
                    }
                });
            }
            Err(err) => {
                ui.error_with_details_on_hover(err.to_string());
            }
        }
    }

    if !any {
        ui.weak("-");
    }
}

/// Would expanding this row reveal anything the one-line summary does not already show?
///
/// True for structs and lists, for several instances, and for several values logged onto the same
/// time point — all of which the summary has to truncate.
fn has_inner_structure(component: ComponentIdentifier, hits: &LatestAllComponentResults) -> bool {
    let Some(unit) = hits.try_as_unit() else {
        // Several values on one time point; expanding lists them separately.
        return true;
    };

    let Some(array) = unit.component_batch_raw(component) else {
        return false;
    };

    1 < array.len() || array.data_type().is_nested()
}
