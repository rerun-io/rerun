// Demonstrates using explicit `CoordinateFrame` with implicit transform frames only.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_hierarchy");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("time", 0);
    rec.log(
        "red_box",
        rerun::Boxes3D::from_half_sizes({{0.5f, 0.5f, 0.5f}}
        ).with_colors({rerun::Color(255, 0, 0)}),
        // Use Transform3D to place the box, so we actually change the underlying coordinate frame and not just the box's pose.
        rerun::Transform3D::from_translation({2.0f, 0.0f, 0.0f})
    );
    rec.log(
        "blue_box",
        rerun::Boxes3D::from_half_sizes({{0.5f, 0.5f, 0.5f}}
        ).with_colors({rerun::Color(0, 0, 255)}),
        // Use Transform3D to place the box, so we actually change the underlying coordinate frame and not just the box's pose.
        rerun::Transform3D::from_translation({-2.0f, 0.0f, 0.0f})
    );
    rec.log("point", rerun::Points3D({{0.0f, 0.0f, 0.0f}}).with_radii({0.5f}));

    // Change where the point is located by cycling through its coordinate frame.
    const char* frame_ids[] = {"tf#/red_box", "tf#/blue_box"};
    for (int t = 0; t < 2; t++) {
        rec.set_time_sequence("time", t + 1); // leave it untouched at t==0.
        rec.log("point", rerun::CoordinateFrame(frame_ids[t]));
    }
}
