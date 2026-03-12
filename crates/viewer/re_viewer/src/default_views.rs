use re_viewer_context::{
    AppOptions, FallbackProviderRegistry, ViewClassRegistry, ViewClassRegistryError,
};

pub fn create_view_class_registry(
    reflection: &re_types_core::reflection::Reflection,
    app_options: &AppOptions,
    fallback_registry: &mut FallbackProviderRegistry,
) -> Result<ViewClassRegistry, ViewClassRegistryError> {
    re_tracing::profile_function!();
    let mut view_class_registry = ViewClassRegistry::default();
    populate_view_class_registry_with_builtin(
        reflection,
        app_options,
        &mut view_class_registry,
        fallback_registry,
    )?;
    Ok(view_class_registry)
}

/// Add built-in views to the registry.
fn populate_view_class_registry_with_builtin(
    reflection: &re_types_core::reflection::Reflection,
    app_options: &AppOptions,
    view_class_registry: &mut ViewClassRegistry,
    fallback_registry: &mut FallbackProviderRegistry,
) -> Result<(), ViewClassRegistryError> {
    re_tracing::profile_function!();
    view_class_registry.add_class::<re_view_bar_chart::BarChartView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_dataframe::DataframeView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_graph::GraphView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    #[cfg(feature = "map_view")]
    view_class_registry.add_class::<re_view_map::MapView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_spatial::SpatialView2D>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_spatial::SpatialView3D>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_tensor::TensorView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_text_document::TextDocumentView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_text_log::TextView>(
        reflection,
        app_options,
        fallback_registry,
    )?;
    view_class_registry.add_class::<re_view_time_series::TimeSeriesView>(
        reflection,
        app_options,
        fallback_registry,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use egui::Vec2;
    use egui_kittest::SnapshotResults;
    use re_chunk::EntityPath;
    use re_test_context::TestContext;
    use re_ui::UiExt as _;
    use re_viewer_context::ViewId;

    use super::*;

    #[test]
    fn test_view_selection_ui() {
        let view_id = ViewId::random();
        let mut test_context = TestContext::new();
        test_context
            .query_results
            .insert(view_id, Default::default());

        test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
        re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

        let view_class_registry = create_view_class_registry(
            &test_context.reflection,
            &test_context.app_options,
            &mut test_context.component_fallback_registry,
        )
        .unwrap();
        let mut snapshot_results = SnapshotResults::new();
        for egui_theme in [egui::Theme::Light, egui::Theme::Dark] {
            for entry in view_class_registry.iter_registry() {
                let class = &entry.class;
                let mut state = class.new_state();
                let space_origin = EntityPath::root();

                let mut did_run = false;

                let mut harness = test_context
                    .setup_kittest_for_rendering_ui([400.0, 700.0])
                    .build_ui(|ui| {
                        ui.set_theme(egui_theme);

                        test_context.run_ui(ui, |viewer_ctx, ui| {
                            ui.set_min_size(Vec2::new(400.0, 300.0));
                            ui.list_item_scope(entry.identifier, |ui| {
                                class
                                    .selection_ui(
                                        viewer_ctx,
                                        ui,
                                        state.as_mut(),
                                        &space_origin,
                                        view_id,
                                    )
                                    .expect("Failed to run selection_ui");
                                did_run = true;
                            });
                        });
                    });

                harness.run();

                let snapshot_options = re_ui::testing::default_snapshot_options_for_ui()
                    .output_path(format!(
                        "tests/snapshots/all_view_selection_uis/{egui_theme:?}"
                    ));
                harness.snapshot_options(entry.identifier.to_string(), &snapshot_options);
                snapshot_results.extend_harness(&mut harness);

                drop(harness);

                assert!(did_run);
            }
        }
    }
}
