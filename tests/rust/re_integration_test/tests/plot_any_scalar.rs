// //! Integration tests for logging arbitrary scalar types using `DynamicArchetype`.
// //!
// //! This test demonstrates visualizing custom scalar data (Float64, Float32) in TimeSeriesViews.

// use std::f64::consts::PI;
// use std::sync::Arc;

// use re_integration_test::HarnessExt as _;
// use re_sdk::archetypes::{self, Scalars};
// use re_sdk::blueprint::components as bp_components;
// use re_sdk::blueprint::datatypes as bp_datatypes;
// use re_sdk::log::RowId;
// use re_sdk::{EntityPath, Timeline};
// use re_viewer::external::arrow;
// use re_viewer::external::re_sdk::DynamicArchetype;
// use re_viewer::external::re_sdk::datatypes::Rgba32;
// use re_viewer::external::re_viewer_context::ViewClass as _;
// use re_viewer::viewer_test_utils::{self, HarnessOptions};
// use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId};
// use re_viewport_blueprint::ViewBlueprint;

// const NUM_POINTS: i64 = 100;

// /// Calculate cos value for given x
// fn cos_curve(x: f64) -> f64 {
//     x.cos()
// }

// /// Calculate sigmoid value for given x
// fn sigmoid_curve(x: f32) -> f32 {
//     5.0 / (1.0 + (-x).exp())
// }

// /// Calculate sine-based value
// fn sine_curve(t: f32) -> f32 {
//     f32::midpoint((t * 1.5).sin(), 1.0) * 5.0
// }

// /// Calculate color value (rainbow effect)
// fn color_curve_1(t: f32) -> u32 {
//     let r = (f32::midpoint((t + 0.0).sin(), 1.0) * 255.0) as u8;
//     let g = (f32::midpoint((t + std::f32::consts::TAU / 3.0).sin(), 1.0) * 255.0) as u8;
//     let b = (f32::midpoint((t + std::f32::consts::TAU / 3.0 * 2.0).sin(), 1.0) * 255.0) as u8;
//     Rgba32::from_rgb(r, g, b).into()
// }

// /// Calculate color value (pink variation)
// fn color_curve_2(t: f32) -> u32 {
//     let x = (f32::midpoint((t * 6.0).sin(), 1.0) * 200.0) as u8;
//     Rgba32::from_rgb(255, x, 255).into()
// }

// /// Test that arbitrary scalar types can be logged and visualized in TimeSeriesView.
// ///
// /// This test verifies that:
// /// 1. DynamicArchetype can be used to log Float64 and Float32 scalar data
// /// 2. Custom component names are properly displayed
// /// 3. TimeSeriesView can visualize the data correctly
// #[tokio::test(flavor = "multi_thread")]
// pub async fn test_any_scalar_gabor() {
//     let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
//         window_size: Some(egui::Vec2::new(1280.0, 800.0)),
//         max_steps: Some(100),
//         ..Default::default()
//     });
//     harness.init_recording();
//     harness.set_blueprint_panel_opened(true);
//     harness.set_selection_panel_opened(false);
//     harness.set_time_panel_opened(true);

//     let timeline = Timeline::new_sequence("step");

//     // Log scalar data using DynamicArchetype
//     for i in 0..NUM_POINTS {
//         // Calculate x value in range [0, 2π] for sin/cos
//         let x_f64 = (i as f64 / NUM_POINTS as f64) * 2.0 * PI;

//         // Calculate x value in range [-6, 6] for sigmoid
//         let x_f32 = ((i as f32 / NUM_POINTS as f32) - 0.5) * 12.0;

//         // Float64 values (cos curve and scaled version)
//         let cos_value = cos_curve(x_f64);
//         let cos_scaled_value = cos_curve(x_f64 + 1.0).abs() * 0.5;

//         // Float32 values (sigmoid and sine curves)
//         let sigmoid_value = sigmoid_curve(x_f32);
//         let sine_value = sine_curve(x_f32);
//         let color_1_value = color_curve_1(x_f32);
//         let color_2_value = color_curve_2(x_f32);

//         // Create Arrow arrays
//         let cos_array = Arc::new(arrow::array::Float64Array::from(vec![cos_value]));
//         let cos_scaled_array = Arc::new(arrow::array::Float64Array::from(vec![cos_scaled_value]));
//         let sigmoid_array = Arc::new(arrow::array::Float32Array::from(vec![sigmoid_value]));
//         let sine_array = Arc::new(arrow::array::Float32Array::from(vec![sine_value]));
//         let color_1_array = Arc::new(arrow::array::UInt32Array::from(vec![color_1_value]));
//         let color_2_array = Arc::new(arrow::array::UInt32Array::from(vec![color_2_value]));

//         // Log Float64 archetype with cos values
//         let float64_archetype = DynamicArchetype::new("MyCustomData")
//             .with_component_from_data("value_1", cos_array)
//             .with_component_from_data("value_2", cos_scaled_array)
//             .with_component_from_data("sigmoid", sigmoid_array.clone());

//         // Log custom archetype with additional data
//         let custom_archetype = DynamicArchetype::new("OtherStuff")
//             .with_component_from_data("sine", sine_array)
//             .with_component_from_data("rainbow_dash", color_1_array)
//             .with_component_from_data("pinkie_pie", color_2_array);

//         harness.log_entity("float64", |builder| {
//             builder
//                 .with_archetype(RowId::new(), [(timeline, i)], &float64_archetype)
//                 .with_archetype(RowId::new(), [(timeline, i)], &custom_archetype)
//         });
//     }

//     // Set up TimeSeriesView to visualize the data
//     harness.clear_current_blueprint();
//     harness.setup_viewport_blueprint(|ctx, blueprint| {
//         let mut view = ViewBlueprint::new_with_root_wildcard(
//             re_view_time_series::TimeSeriesView::identifier(),
//         );
//         view.display_name = Some("Scalar Plot".into());
//         let view_id = blueprint.add_view_at_root(view);

//         let mappings = vec![bp_components::VisualizerComponentMapping(
//             bp_datatypes::VisualizerComponentMapping {
//                 selector: "MyCustomData:value_1".into(),
//                 target: "Scalar:Scalar".into(),
//             },
//         )];

//         let mut visualizer: re_sdk_types::Visualizer =
//             (&archetypes::SeriesPoints::default().with_colors([(0, 255, 0)])).into();
//         visualizer = visualizer.with_mappings(mappings);

//         ctx.save_visualizers(&EntityPath::from("float64"), view_id, [visualizer]);
//     });

//     // Save overrides for the view

//     harness.snapshot_app("any_scalar_gabor_1");

//     // Expand the blueprint tree to show entities
//     harness.blueprint_tree().right_click_label("Scalar Plot");
//     harness.click_label("Expand all");
//     harness.snapshot_app("any_scalar_gabor_2");

//     // Click on the entity in the streams tree to show its properties
//     harness.streams_tree().click_label("float64");
//     harness.set_selection_panel_opened(true);
//     harness.snapshot_app("any_scalar_gabor_3");
// }
