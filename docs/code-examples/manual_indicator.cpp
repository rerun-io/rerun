// Shows how to manually associate one or more indicator components with arbitrary data.

#include <rerun.hpp>

#include <vector>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_manual_indicator");
    rec.spawn().exit_on_failure();

    std::vector<rerun::Position3D> positions = {
        {0.0, 0.0, 0.0},
        {10.0, 0.0, 0.0},
        {0.0, 10.0, 0.0},
    };
    std::vector<rerun::Color> colors = {
        {255, 0, 0},
        {0, 255, 0},
        {0, 0, 255},
    };
    std::vector<rerun::Radius> radii = {1.0};

    // Specify both a Mesh3D and a Points3D indicator component so that the data is shown as both a
    // 3D mesh _and_ a point cloud by default.
    rec.log(
        "points_and_mesh",
        rerun::Points3D::IndicatorComponent(),
        rerun::Mesh3D::IndicatorComponent(),
        positions,
        colors,
        radii
    );
}
