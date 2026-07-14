//! Use a blueprint to customize a map view.

use rerun::blueprint::{
    components as blueprint_components, Blueprint, MapView,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        MapView::new("MapView")
            .with_origin("points")
            .with_zoom(16.0)
            .with_background(blueprint_components::MapProvider::OpenStreetMap),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_map_view")
        .with_blueprint(blueprint)
        .spawn()?;

    rec.log(
        "points",
        &rerun::GeoPoints::from_lat_lon([
            (47.6344, 19.1397),
            (47.6334, 19.1399),
        ])
        .with_radii([rerun::Radius::new_ui_points(20.0)]),
    )?;

    Ok(())
}
