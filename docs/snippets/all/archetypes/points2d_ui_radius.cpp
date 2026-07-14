// Log some points with ui points & scene unit radii.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_points2d_ui_radius");
    rec.spawn().exit_on_failure();

    // Two blue points with scene unit radii of 0.1 and 0.3.
    rec.log(
        "scene_units",
        rerun::Points2D({{0.0f, 0.0f}, {0.0f, 1.0f}})
            // By default, radii are interpreted as world-space units.
            .with_radii({0.1f, 0.3f})
            .with_colors(rerun::Color(0, 0, 255))
    );

    // Two red points with ui point radii of 40 and 60.
    // UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
    // For 100% ui scaling, UI points are equal to pixels.
    rec.log(
        "ui_points",
        rerun::Points2D({{1.0f, 0.0f}, {1.0f, 1.0f}})
            // rerun::Radius::ui_points produces radii that the viewer interprets as given in ui points.
            .with_radii({
                rerun::Radius::ui_points(40.0f),
                rerun::Radius::ui_points(60.0f),
            })
            .with_colors(rerun::Color(255, 0, 0))
    );

    // TODO(#5521): log VisualBounds2D
}
