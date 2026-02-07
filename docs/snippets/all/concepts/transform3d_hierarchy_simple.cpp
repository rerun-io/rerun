//! Logs a simple transform hierarchy.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_hierarchy_simple");
    rec.spawn().exit_on_failure();

    // Log entities at their hierarchy positions.
    rec.log(
        "sun",
        rerun::Ellipsoids3D::from_half_sizes({{1.0f, 1.0f, 1.0f}})
            .with_colors(rerun::Color(255, 200, 10))
            .with_fill_mode(rerun::FillMode::Solid)
    );

    rec.log(
        "sun/planet",
        rerun::Ellipsoids3D::from_half_sizes({{0.4f, 0.4f, 0.4f}})
            .with_colors(rerun::Color(40, 80, 200))
            .with_fill_mode(rerun::FillMode::Solid)
    );

    rec.log(
        "sun/planet/moon",
        rerun::Ellipsoids3D::from_half_sizes({{0.15f, 0.15f, 0.15f}})
            .with_colors(rerun::Color(180, 180, 180))
            .with_fill_mode(rerun::FillMode::Solid)
    );

    // Define transforms - each describes the relationship to its parent.
    rec.log(
        "sun/planet",
        rerun::Transform3D::from_translation({6.0f, 0.0f, 0.0f})
    ); // Planet 6 units from sun.

    rec.log(
        "sun/planet/moon",
        rerun::Transform3D::from_translation({3.0f, 0.0f, 0.0f})
    ); // Moon 3 units from planet.

    return 0;
}
