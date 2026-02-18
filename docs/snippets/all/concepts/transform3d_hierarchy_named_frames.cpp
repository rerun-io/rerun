//! Logs a simple transform hierarchy with named frames.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_hierarchy_named_frames");
    rec.spawn().exit_on_failure();

    // Define entities with explicit coordinate frames.
    rec.log(
        "sun",
        rerun::Ellipsoids3D::from_half_sizes({{1.0f, 1.0f, 1.0f}})
            .with_colors(rerun::Color(255, 200, 10))
            .with_fill_mode(rerun::FillMode::Solid),
        rerun::CoordinateFrame("sun_frame")
    );

    rec.log(
        "planet",
        rerun::Ellipsoids3D::from_half_sizes({{0.4f, 0.4f, 0.4f}})
            .with_colors(rerun::Color(40, 80, 200))
            .with_fill_mode(rerun::FillMode::Solid),
        rerun::CoordinateFrame("planet_frame")
    );

    rec.log(
        "moon",
        rerun::Ellipsoids3D::from_half_sizes({{0.15f, 0.15f, 0.15f}})
            .with_colors(rerun::Color(180, 180, 180))
            .with_fill_mode(rerun::FillMode::Solid),
        rerun::CoordinateFrame("moon_frame")
    );

    // Define explicit frame relationships.
    rec.log(
        "planet_transform",
        rerun::Transform3D::from_translation({6.0f, 0.0f, 0.0f})
            .with_child_frame("planet_frame")
            .with_parent_frame("sun_frame")
    );

    rec.log(
        "moon_transform",
        rerun::Transform3D::from_translation({3.0f, 0.0f, 0.0f})
            .with_child_frame("moon_frame")
            .with_parent_frame("planet_frame")
    );

    // Connect the viewer to the sun's coordinate frame.
    // This is only needed in the absence of blueprints since a default view will typically be created at `/`.
    rec.log_static("/", rerun::CoordinateFrame("sun_frame"));

    return 0;
}
