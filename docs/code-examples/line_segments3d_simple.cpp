// Log a simple set of line segments.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_segments3d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // TODO(#3202): I want to do this!
    // std::vector<std::vector<rerun::datatypes::Vec3D>> points = {
    //     {{0.f, 0.f, 0.f}, {0.f, 0.f, 1.f}},
    //     {{1.f, 0.f, 0.f}, {1.f, 0.f, 1.f}},
    //     {{1.f, 1.f, 0.f}, {1.f, 1.f, 1.f}},
    //     {{0.f, 1.f, 0.f}, {0.f, 1.f, 1.f}},
    // };
    // rec.log("segments", rerun::LineStrips3D(points));

    std::vector<rerun::datatypes::Vec3D> points1 = {{0.f, 0.f, 0.f}, {0.f, 0.f, 1.f}};
    std::vector<rerun::datatypes::Vec3D> points2 = {{1.f, 0.f, 0.f}, {1.f, 0.f, 1.f}};
    std::vector<rerun::datatypes::Vec3D> points3 = {{1.f, 1.f, 0.f}, {1.f, 1.f, 1.f}};
    std::vector<rerun::datatypes::Vec3D> points4 = {{0.f, 1.f, 0.f}, {0.f, 1.f, 1.f}};
    rec.log("segments", rerun::LineStrips3D({points1, points2, points3, points4}));
}
