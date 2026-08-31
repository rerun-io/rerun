//! Log scalar measurements with variances over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_measurements_simple")
            .spawn()?;

    // Two parallel pressure sensors (in Pa), each with slowly drifting variance.
    for step in 0..64 {
        rec.set_time_sequence("step", step);
        let pressures = [
            101_325.0 + 50.0 * (step as f64 / 10.0).sin(),
            101_300.0 + 30.0 * (step as f64 / 8.0).cos(),
        ];
        let variances = [
            100.0 + 25.0 * (step as f64 / 7.0).sin(),
            80.0 + 15.0 * (step as f64 / 11.0).cos(),
        ];
        rec.log(
            "pressure",
            &rerun::Measurements::new(pressures)
                .with_variances(variances)
                .with_units(["Pa"]),
        )?;
    }

    Ok(())
}
