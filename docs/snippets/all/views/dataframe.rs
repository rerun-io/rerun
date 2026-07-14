//! Use a blueprint to customize a DataframeView.

use rerun::blueprint::{
    archetypes as blueprint_archetypes, Blueprint, DataframeView,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        DataframeView::new("Dataframe")
            .with_origin("/trig")
            .with_query(
                &blueprint_archetypes::DataframeQuery::new()
                    .with_timeline("t")
                    .with_auto_scroll(true),
            ),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dataframe")
        .with_blueprint(blueprint)
        .spawn()?;

    for t in 0..((std::f64::consts::PI * 4.0 * 100.0) as i64) {
        rec.set_duration_secs("t", t as f64);
        rec.log(
            "trig/sin",
            &rerun::Scalars::single((t as f64 / 100.0).sin()),
        )?;
        rec.log(
            "trig/cos",
            &rerun::Scalars::single((t as f64 / 100.0).cos()),
        )?;

        if t % 5 == 0 {
            rec.log(
                "trig/tan_sparse",
                &rerun::Scalars::single((t as f64 / 100.0).tan()),
            )?;
        }
    }

    Ok(())
}
