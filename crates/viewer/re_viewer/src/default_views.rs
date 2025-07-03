use re_viewer_context::{ViewClassRegistry, ViewClassRegistryError};

pub fn create_view_class_registry() -> Result<ViewClassRegistry, ViewClassRegistryError> {
    re_tracing::profile_function!();
    let mut view_class_registry = ViewClassRegistry::default();
    populate_view_class_registry_with_builtin(&mut view_class_registry)?;
    Ok(view_class_registry)
}

/// Add built-in views to the registry.
fn populate_view_class_registry_with_builtin(
    view_class_registry: &mut ViewClassRegistry,
) -> Result<(), ViewClassRegistryError> {
    re_tracing::profile_function!();
    view_class_registry.add_class::<re_view_bar_chart::BarChartView>()?;
    view_class_registry.add_class::<re_view_dataframe::DataframeView>()?;
    view_class_registry.add_class::<re_view_graph::GraphView>()?;
    #[cfg(feature = "map_view")]
    view_class_registry.add_class::<re_view_map::MapView>()?;
    view_class_registry.add_class::<re_view_spatial::SpatialView2D>()?;
    view_class_registry.add_class::<re_view_spatial::SpatialView3D>()?;
    view_class_registry.add_class::<re_view_tensor::TensorView>()?;
    view_class_registry.add_class::<re_view_text_document::TextDocumentView>()?;
    view_class_registry.add_class::<re_view_text_log::TextView>()?;
    view_class_registry.add_class::<re_view_time_series::TimeSeriesView>()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use egui::Vec2;
    use egui_kittest::SnapshotOptions;
    use re_chunk::EntityPath;
    use re_ui::UiExt as _;
    use re_viewer_context::{ViewId, test_context::TestContext};

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

        let view_class_registry = create_view_class_registry().unwrap();
        for egui_theme in [egui::Theme::Light, egui::Theme::Dark] {
            for entry in view_class_registry.iter_registry() {
                let class = &entry.class;
                let mut state = class.new_state();
                let space_origin = EntityPath::root();

                let mut did_run = false;

                let mut harness = test_context
                    .setup_kittest_for_rendering()
                    .with_size([400.0, 640.0])
                    .build(|egui_ctx| {
                        re_ui::apply_style_and_install_loaders(egui_ctx);
                        egui_ctx.set_theme(egui_theme);

                        egui::CentralPanel::default().show(egui_ctx, |ui| {
                            test_context.run(egui_ctx, |viewer_ctx| {
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
                    });

                harness.run();

                let snapshot_options = SnapshotOptions::default().output_path(format!(
                    "tests/snapshots/all_view_selecion_uis/{egui_theme:?}"
                ));
                harness.snapshot_options(&entry.identifier, &snapshot_options);

                drop(harness);

                assert!(did_run);
            }
        }
    }
}
