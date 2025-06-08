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
