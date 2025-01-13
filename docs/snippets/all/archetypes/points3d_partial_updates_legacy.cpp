// Demonstrates usage of the legacy partial updates APIs.

#include <rerun.hpp>

#include <algorithm>
#include <vector>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_points3d_partial_updates_legacy");
    rec.spawn().exit_on_failure();

    std::vector<rerun::Position3D> positions;
    for (int i = 0; i < 10; ++i) {
        positions.emplace_back(static_cast<float>(i), 0.0f, 0.0f);
    }

    rec.set_time_sequence("frame", 0);
    rec.log("points", rerun::Points3D(positions));

    for (int i = 0; i < 10; ++i) {
        std::vector<rerun::Color> colors;
        for (int n = 0; n < 10; ++n) {
            if (n < i) {
                colors.emplace_back(rerun::Color(20, 200, 20));
            } else {
                colors.emplace_back(rerun::Color(200, 20, 20));
            }
        }

        std::vector<rerun::Radius> radii;
        for (int n = 0; n < 10; ++n) {
            if (n < i) {
                radii.emplace_back(rerun::Radius(0.6f));
            } else {
                radii.emplace_back(rerun::Radius(0.2f));
            }
        }

        rec.set_time_sequence("frame", i);
        rec.log("points", colors, radii);
    }

    std::vector<rerun::Radius> radii;
    radii.emplace_back(0.3f);

    rec.set_time_sequence("frame", 20);
    rec.log(
        "points",
        rerun::Points3D::IndicatorComponent(),
        positions,
        radii,
        std::vector<rerun::components::Color>(),
        std::vector<rerun::components::Text>(),
        std::vector<rerun::components::ShowLabels>(),
        std::vector<rerun::components::ClassId>(),
        std::vector<rerun::components::KeypointId>()
    );
}
