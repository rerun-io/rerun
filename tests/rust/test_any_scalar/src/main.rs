//! Minimal example demonstrating different scalar types that can be visualized in the viewer.
//! TODO(#10631): ⚠️ This is a work in progress and should eventually become an integration test.

use std::f64::consts::PI;
use std::sync::Arc;

use rerun::external::arrow;
use rerun::external::re_log;
use rerun::{DynamicArchetype, RecordingStream, Scalars};

const NUM_POINTS: i64 = 100;

/// Calculate sin value for given x
fn sin_curve(x: f64) -> f64 {
    x.sin()
}

/// Calculate cos value for given x
fn cos_curve(x: f64) -> f64 {
    x.cos()
}

/// Calculate sigmoid value for given x
fn sigmoid_curve(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn log_scalar_data(rec: &RecordingStream) -> anyhow::Result<()> {
    for i in 0..NUM_POINTS {
        // Calculate x value in range [0, 2π] for sin/cos
        let x_f64 = (i as f64 / NUM_POINTS as f64) * 2.0 * PI;

        // Calculate x value in range [-6, 6] for sigmoid
        let x_f32 = ((i as f32 / NUM_POINTS as f32) - 0.5) * 12.0;

        // Set time for this iteration
        rec.set_time_sequence("step", i);

        // 1. Log using builtin Scalars archetype with sin curve
        let sin_value = sin_curve(x_f64);
        rec.log("builtin", &Scalars::new([sin_value]))?;

        // 2. Log using DynamicArchetype with Float64 (cos curve)
        let cos_value = cos_curve(x_f64);
        let cos_array = Arc::new(arrow::array::Float64Array::from(vec![cos_value]));
        let cos_scaled_array = Arc::new(arrow::array::Float64Array::from(vec![cos_value * 0.5]));

        // 3. Log using DynamicArchetype with Float32 (sigmoid curve)
        let sigmoid_value = sigmoid_curve(x_f32);
        let sigmoid_array = Arc::new(arrow::array::Float32Array::from(vec![sigmoid_value]));
        let sigmoid_scaled_array =
            Arc::new(arrow::array::Float32Array::from(vec![sigmoid_value * 0.5]));

        let float32_archetype = DynamicArchetype::new("Float32Scalars")
            .with_component_from_data("sigmoid", sigmoid_array.clone())
            .with_component_from_data("sigmoid_scaled", sigmoid_scaled_array);

        let float64_archetype = DynamicArchetype::new("Float64Scalars")
            .with_component_from_data("cos", cos_array)
            .with_component_from_data("cos_scaled", cos_scaled_array)
            .with_component_from_data("sigmoid", sigmoid_array);

        rec.log("float64", &float64_archetype)?;

        rec.log("float32", &float32_archetype)?;

        // 4. Log using DynamicArchetype with Float64 array containing both original and scaled
        let cos_multi_array = Arc::new(arrow::array::Float64Array::from(vec![
            cos_value + 0.5,
            cos_value * 0.5 + 0.5,
        ]));

        let float64_multi_archetype = DynamicArchetype::new("Float64MultiScalars")
            .with_component_from_data("cos", cos_multi_array);

        rec.log("float64/multi", &float64_multi_archetype)?;

        // 5. Log using DynamicArchetype with Float32 array containing both original and scaled
        let sigmoid_multi_array = Arc::new(arrow::array::Float32Array::from(vec![
            sigmoid_value + 0.5,
            sigmoid_value * 0.5 + 0.5,
        ]));

        let float32_multi_archetype = DynamicArchetype::new("Float32MultiScalars")
            .with_component_from_data("sigmoid", sigmoid_multi_array);

        rec.log("float32/multi", &float32_multi_archetype)?;
    }

    Ok(())
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_test_any_scalar")?;
    log_scalar_data(&rec)?;

    Ok(())
}
