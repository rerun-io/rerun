//! Log a geospatial polygon with a hole.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_geo_polygons")
        .spawn()?;
    let polygon = rerun::components::GeoPolygon::from_lat_lon([
        vec![[48.85, 2.28], [48.85, 2.36], [48.90, 2.36], [48.90, 2.28]],
        vec![[48.86, 2.30], [48.86, 2.33], [48.88, 2.33], [48.88, 2.30]],
    ]);
    rec.log(
        "region",
        &rerun::GeoPolygons::new([polygon]).with_colors([
            rerun::Color::from_unmultiplied_rgba(40, 120, 230, 128),
        ]),
    )?;
    Ok(())
}
