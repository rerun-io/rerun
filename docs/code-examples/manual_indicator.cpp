// Shows how to manually associate one or more indicator components with arbitrary data.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_manual_indicator");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Position3D> positions = {
        {0.0, 0.0, 0.0},
        {10.0, 0.0, 0.0},
        {0.0, 10.0, 0.0},
    };
    std::vector<rr::components::Color> colors = {
        {255, 0, 0},
        {0, 255, 0},
        {0, 0, 255},
    };
    std::vector<rr::components::Radius> radii = {1.0};

    // Specify both a Mesh3D and a Points3D indicator component so that the data is shown as both a
    // 3D mesh _and_ a point cloud by default.
    rec.log(
        "points_and_mesh",
        rr::Points3D::IndicatorComponent(),
        rr::Mesh3D::IndicatorComponent(),
        positions,
        colors,
        radii
    );
}
