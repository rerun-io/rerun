// Log a geospatial polygon with a hole.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_geo_polygons");
    rec.spawn().exit_on_failure();
    auto polygon = rerun::components::GeoPolygon::from_lat_lon({
        {{48.85, 2.28}, {48.85, 2.36}, {48.90, 2.36}, {48.90, 2.28}},
        {{48.86, 2.30}, {48.86, 2.33}, {48.88, 2.33}, {48.88, 2.30}},
    });
    rec.log(
        "region",
        rerun::GeoPolygons({polygon}).with_colors(
            {rerun::Color(40, 120, 230, 128)}
        )
    );
}
