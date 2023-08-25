// Log some transforms.

#include <rerun.hpp>

#include <cmath>

namespace rr = rerun;
namespace rrd = rr::datatypes;

const float pi = static_cast<float>(M_PI);

int main() {
    auto rr_stream = rr::RecordingStream("rerun-example-transform3d");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    auto arrow = rr::Arrows3D({0.0f, 1.0f, 0.0f});

    rr_stream.log("base", arrow);

    rr_stream.log("base/translated", rr::Transform3D({1.0f, 0.0f, 0.0f}));
    rr_stream.log("base/translated", arrow);

    rr_stream.log(
        "base/rotated_scaled",
        rr::Transform3D(
            rrd::RotationAxisAngle({0.0f, 0.0f, 1.0f}, rrd::Angle::radians(pi / 4.0f)),
            2.0f
        )
    );
    rr_stream.log("base/rotated_scaled", arrow);
}
