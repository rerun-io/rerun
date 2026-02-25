//! Demonstrates how to configure visualizer component mappings from blueprint.

use std::sync::Arc;

use rerun::AsComponents as _;
use rerun::blueprint::VisualizableArchetype as _;
use rerun::external::arrow::array::{Array, Float32Array, Float64Array, StructArray};
use rerun::external::arrow::datatypes::{DataType, Field};

// region: nested_struct
/// Creates a `StructArray` with a `values` field containing sigmoid data.
///
/// Note: We intentionally use `Float32` here to demonstrate that the data will be
/// automatically cast to the correct type (`Float64`) when resolved by the visualizer.
fn make_sigmoid_struct_array(steps: usize) -> StructArray {
    let sigmoid_values: Vec<f32> = (0..steps)
        .map(|step| {
            let x = step as f32 / 10.0;
            1.0 / (1.0 + (-(x - 3.0)).exp())
        })
        .collect();

    StructArray::from(vec![(
        Arc::new(Field::new("values", DataType::Float32, true)),
        Arc::new(Float32Array::from(sigmoid_values)) as Arc<dyn Array>,
    )])
}
// endregion: nested_struct

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_component_mapping").spawn()?;

    // Generate columns for regular scalars (sin)
    let sin = (0..64).map(|step| (step as f64 / 10.0).sin());
    let sin_columns = rerun::Scalars::new(sin).columns_of_unit_batches()?;

    // region: custom_data
    // Generate columns for custom component (cos)
    let cos = (0..64).map(|step| (step as f64 / 10.0).cos());
    let cos_array = Arc::new(cos.collect::<Float64Array>());
    let custom_columns = rerun::DynamicArchetype::new("custom")
        .with_component_from_data("my_custom_scalar", cos_array)
        .as_serialized_batches()
        .into_iter()
        .map(|batch| batch.column_of_unit_batches())
        .collect::<Result<Vec<_>, _>>()?;

    // Generate columns for nested custom component (sigmoid)
    let sigmoid_array = Arc::new(make_sigmoid_struct_array(64));
    let nested_columns = rerun::DynamicArchetype::new("custom")
        .with_component_from_data("my_nested_scalar", sigmoid_array)
        .as_serialized_batches()
        .into_iter()
        .map(|batch| batch.column_of_unit_batches())
        .collect::<Result<Vec<_>, _>>()?;
    // endregion: custom_data

    // Send plot data using send_columns.
    rec.send_columns(
        "plot",
        [rerun::TimeColumn::new_sequence("step", 0..64)],
        sin_columns.chain(custom_columns).chain(nested_columns),
    )?;

    // Add a line series color to the store data
    rec.log_static(
        "plot",
        &rerun::SeriesLines::new().with_colors([[255, 0, 0]]),
    )?;

    // Create a blueprint with explicit component mappings
    let blueprint = rerun::blueprint::Blueprint::new(
        rerun::blueprint::TimeSeriesView::new("Component Mapping Demo")
            .with_origin("/")
            // Set default color for series to green
            .with_defaults(&rerun::SeriesLines::new().with_colors([[0, 255, 0]]))
            .with_overrides(
                "plot",
                [
                    // region: custom_value
                    // Red sine:
                    // * set the name via an override
                    // * explicitly use the view's default for color
                    // * everything else uses the automatic component mappings, so it will pick up scalars from the store
                    rerun::SeriesLines::new()
                        .with_names(["sine (store)"])
                        .visualizer()
                        .with_mappings(vec![
                            rerun::blueprint::VisualizerComponentMapping::new_default(
                                rerun::SeriesLines::descriptor_colors().component,
                            )
                            .into(),
                        ]),
                    // endregion: custom_value
                    // region: source_mapping
                    // Green cosine:
                    // * source scalars from the custom component "plot:my_custom_scalar"
                    // * set the name via an override
                    // * everything else uses the automatic component mappings, so it will pick up colors from the view default
                    rerun::SeriesLines::new()
                        .with_names(["cosine (custom)"])
                        .visualizer()
                        .with_mappings(vec![
                            rerun::blueprint::VisualizerComponentMapping::new_source_component(
                                rerun::Scalars::descriptor_scalars().component,
                                "custom:my_custom_scalar",
                            )
                            .into(),
                        ]),
                    // endregion: source_mapping
                    // region: selector_mapping
                    // Blue sigmoid:
                    // * source scalars from a nested struct using a selector to extract the "values" field
                    // * set the name and an explicit blue color via overrides
                    rerun::SeriesLines::new()
                        .with_names(["sigmoid (nested)"])
                        .with_colors([[0, 0, 255]])
                        .visualizer()
                        .with_mappings(vec![
                            rerun::blueprint::VisualizerComponentMapping::new_source_component_with_selector(
                                "Scalars:scalars",
                                "custom:my_nested_scalar",
                                ".values",
                            )
                            .into(),
                        ]),
                    // endregion: selector_mapping
                ],
            ),
    );

    blueprint.send(&rec, rerun::blueprint::BlueprintActivation::default())?;

    Ok(())
}
