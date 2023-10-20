// Log some transforms.

#include <rerun.hpp>

#include <cmath>

namespace rrd = rerun::datatypes;

const float TAU = static_cast<float>(2.0 * M_PI);

int main() {
    auto rec = rerun::RecordingStream("rerun_example_transform3d");
    rec.connect().throw_on_failure();

    auto arrow =
        rerun::Arrows3D::from_vectors({{0.0f, 1.0f, 0.0f}}).with_origins({{0.0f, 0.0f, 0.0f}});

    rec.log("base", arrow);

    rec.log("base/translated", rerun::Transform3D({1.0f, 0.0f, 0.0f}));
    rec.log("base/translated", arrow);

    rec.log(
        "base/rotated_scaled",
        rerun::Transform3D(
            rrd::RotationAxisAngle({0.0f, 0.0f, 1.0f}, rrd::Angle::radians(TAU / 8.0f)),
            2.0f
        )
    );
    rec.log("base/rotated_scaled", arrow);
}
