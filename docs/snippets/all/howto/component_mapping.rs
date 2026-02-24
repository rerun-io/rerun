//! Demonstrates how to configure visualizer component mappings from blueprint.

use std::sync::Arc;

use rerun::AsComponents as _;
use rerun::blueprint::VisualizableArchetype as _;
use rerun::external::arrow::array::Float64Array;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_component_mapping").spawn()?;

    // Generate columns for regular scalars (sin)
    let sin = (0..64).map(|step| (step as f64 / 10.0).sin());
    let sin_columns = rerun::Scalars::new(sin).columns_of_unit_batches()?;

    // Generate columns for custom component (cos)
    let cos = (0..64).map(|step| (step as f64 / 10.0).cos());
    let cos_array = Arc::new(cos.collect::<Float64Array>());
    let custom_columns = rerun::DynamicArchetype::new("custom")
        .with_component_from_data("my_custom_scalar", cos_array)
        .as_serialized_batches()
        .into_iter()
        .map(|batch| batch.column_of_unit_batches())
        .collect::<Result<Vec<_>, _>>()?;

    // Send plot data using send_columns.
    rec.send_columns(
        "plot",
        [rerun::TimeColumn::new_sequence("step", 0..64)],
        sin_columns.chain(custom_columns),
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
                    // Blue cosine:
                    // * source scalars from the custom component "plot:my_custom_scalar"
                    // * set the name via an override
                    // * everything else uses the automatic component mappings, so it will pick up colors from the view default
                    // region: source_mapping
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
                ],
            ),
    );

    blueprint.send(&rec, rerun::blueprint::BlueprintActivation::default())?;

    Ok(())
}
