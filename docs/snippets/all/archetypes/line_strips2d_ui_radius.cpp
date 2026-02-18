// Log lines with ui points & scene unit radii.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_line_strip2d_ui_radius");
    rec.spawn().exit_on_failure();

    // A blue line with a scene unit radii of 0.01.
    rerun::LineStrip2D linestrip_blue({{0.f, 0.f}, {0.f, 1.f}, {1.f, 0.f}, {1.f, 1.f}});
    rec.log(
        "scene_unit_line",
        rerun::LineStrips2D(linestrip_blue)
            // By default, radii are interpreted as world-space units.
            .with_radii(0.01f)
            .with_colors(rerun::Color(0, 0, 255))
    );

    // A red line with a ui point radii of 5.
    // UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
    // For 100 % ui scaling, UI points are equal to pixels.
    rerun::LineStrip2D linestrip_red({{3.f, 0.f}, {3.f, 1.f}, {4.f, 0.f}, {4.f, 1.f}});
    rec.log(
        "ui_points_line",
        rerun::LineStrips2D(linestrip_red)
            // By default, radii are interpreted as world-space units.
            .with_radii(rerun::Radius::ui_points(5.0f))
            .with_colors(rerun::Color(255, 0, 0))
    );

    // TODO(#5520): log VisualBounds2D
}
