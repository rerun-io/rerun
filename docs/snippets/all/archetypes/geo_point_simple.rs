//! Log some very simple geospatial point.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_geo_points").spawn()?;

    rec.log(
        "rerun_hq",
        &rerun::GeoPoints::from_lat_lon([(59.319221, 18.075631)])
            .with_radii([rerun::Radius::new_ui_points(10.0)])
            .with_colors([rerun::Color::from_rgb(255, 0, 0)]),
    )?;

    Ok(())
}
