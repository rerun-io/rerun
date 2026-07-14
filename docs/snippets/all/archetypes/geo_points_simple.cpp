// Log some very simple geospatial point.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_geo_points");
    rec.spawn().exit_on_failure();

    rec.log(
        "rerun_hq",
        rerun::GeoPoints::from_lat_lon({{59.319221, 18.075631}})
            .with_radii(rerun::Radius::ui_points(10.0f))
            .with_colors(rerun::Color(255, 0, 0))
    );
}
