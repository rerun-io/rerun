// Log a simple geospatial line string.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_geo_line_strings");
    rec.spawn().exit_on_failure();

    auto line_string = rerun::components::GeoLineString::from_lat_lon(
        {{41.0000, -109.0452},
         {41.0000, -102.0415},
         {36.9931, -102.0415},
         {36.9931, -109.0452},
         {41.0000, -109.0452}}
    );

    rec.log(
        "colorado",
        rerun::GeoLineStrings(line_string)
            .with_radii(rerun::Radius::ui_points(2.0f))
            .with_colors(rerun::Color(0, 0, 255))
    );
}
