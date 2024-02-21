// Logs a point cloud and a perspective camera looking at it.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_pinhole_perspective");
    rec.spawn().exit_on_failure();

    const float fov_y = 0.7853982f;
    const float aspect_ratio = 1.7777778f;
    rec.log(
        "world/cam",
        rerun::Pinhole::from_fov_and_aspect_ratio(fov_y, aspect_ratio)
            .with_camera_xyz(rerun::components::ViewCoordinates::RUB)
    );

    rec.log(
        "world/points",
        rerun::Points3D({{0.0f, 0.0f, -0.5f}, {0.1f, 0.1f, -0.5f}, {-0.1f, -0.1f, -0.5f}})
    );
}
