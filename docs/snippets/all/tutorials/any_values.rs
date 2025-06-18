//! Log arbitrary data.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_any_values").spawn()?;

    let any_values = rerun::AnyValues::default()
        // Using Rerun's builtin components.
        .with_component::<rerun::components::Scalar>("confidence", [1.2, 3.4, 5.6])
        .with_component::<rerun::components::Text>("description", vec!["Bla bla bla…"])
        // Using arbitrary Arrow data.
        .with_field(
            "homepage",
            Arc::new(arrow::array::StringArray::from(vec![
                "https://www.rerun.io",
            ])),
        )
        .with_field(
            "repository",
            Arc::new(arrow::array::StringArray::from(vec![
                "https://github.com/rerun-io/rerun",
            ])),
        );

    rec.log("any_values", &any_values)?;

    Ok(())
}
