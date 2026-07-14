//! Use a blueprint to customize a Spatial2DView.

use rerun::blueprint::{
    Blueprint, Spatial2DView, archetypes as blueprint_archetypes,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        Spatial2DView::new("2D Scene")
            .with_origin("/")
            .with_background(rerun::Color::from_rgb(105, 20, 105))
            .with_visual_bounds(blueprint_archetypes::VisualBounds2D::new(
                rerun::datatypes::Range2D {
                    x_range: [-5.0, 5.0].into(),
                    y_range: [-5.0, 5.0].into(),
                },
            )),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_spatial_2d")
        .with_blueprint(blueprint)
        .spawn()?;

    let n = 150;
    let positions = (0..n).map(|i| {
        let t = i as f64 / (n - 1) as f64;
        let angle = t * 10.0 * std::f64::consts::PI;
        let radius = (t * 3.0).powi(2);
        ((angle.cos() * radius) as f32, (angle.sin() * radius) as f32)
    });
    let colors = (0..n).map(|i| {
        let t = i as f64 / (n - 1) as f64;
        [255, (255.0 * (1.0 - t)) as u8, (255.0 * t) as u8]
    });
    let radii = (0..n).map(|i| {
        let t = i as f64 / (n - 1) as f64;
        (0.01 + (0.7 - 0.01) * t) as f32
    });

    rec.log(
        "points",
        &rerun::Points2D::new(positions)
            .with_colors(colors)
            .with_radii(radii),
    )?;

    Ok(())
}
