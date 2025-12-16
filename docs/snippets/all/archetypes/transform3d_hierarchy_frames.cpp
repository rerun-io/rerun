// Logs a transform hierarchy using named transform frame relationships.

#include <rerun.hpp>

constexpr float TAU = 6.28318530717958647692528676655900577f;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_hierarchy_frames");
    rec.spawn().exit_on_failure();

    rec.set_time_duration_secs("sim_time", 0.0);

    // Planetary motion is typically in the XY plane.
    rec.log_static("/", rerun::ViewCoordinates::RIGHT_HAND_Z_UP);

    // Setup spheres, all are in the center of their own space:
    rec.log(
        "sun",
        rerun::Ellipsoids3D::from_centers_and_half_sizes({{0.0f, 0.0f, 0.0f}}, {{1.0f, 1.0f, 1.0f}})
            .with_colors(rerun::Color(255, 200, 10))
            .with_fill_mode(rerun::components::FillMode::Solid),
        rerun::CoordinateFrame("sun_frame")
    );

    rec.log(
        "planet",
        rerun::Ellipsoids3D::from_centers_and_half_sizes({{0.0f, 0.0f, 0.0f}}, {{0.4f, 0.4f, 0.4f}})
            .with_colors(rerun::Color(40, 80, 200))
            .with_fill_mode(rerun::components::FillMode::Solid),
        rerun::CoordinateFrame("planet_frame")
    );

    rec.log(
        "moon",
        rerun::Ellipsoids3D::from_centers_and_half_sizes(
            {{0.0f, 0.0f, 0.0f}},
            {{0.15f, 0.15f, 0.15f}}
        )
            .with_colors(rerun::Color(180, 180, 180))
            .with_fill_mode(rerun::components::FillMode::Solid),
        rerun::CoordinateFrame("moon_frame")
    );

    // The viewer automatically creates a 3D view at `/`. To connect it to our transform hierarchy, we set its coordinate frame
    // to `sun_frame` as well. Alternatively, we could also set a blueprint that makes `/sun` the space origin.
    rec.log("/", rerun::CoordinateFrame("sun_frame"));

    // Draw fixed paths where the planet & moon move.
    float d_planet = 6.0f;
    float d_moon = 3.0f;
    std::vector<std::array<float, 3>> planet_path, moon_path;
    for (int i = 0; i <= 100; i++) {
        float angle = static_cast<float>(i) * 0.01f * TAU;
        float circle_x = std::sin(angle);
        float circle_y = std::cos(angle);
        planet_path.push_back({circle_x * d_planet, circle_y * d_planet, 0.0f});
        moon_path.push_back({circle_x * d_moon, circle_y * d_moon, 0.0f});
    }
    rec.log(
        "planet_path",
        rerun::LineStrips3D(rerun::LineStrip3D(planet_path)),
        rerun::CoordinateFrame("sun_frame")
    );
    rec.log(
        "moon_path",
        rerun::LineStrips3D(rerun::LineStrip3D(moon_path)),
        rerun::CoordinateFrame("planet_frame")
    );

    // Movement via transforms.
    for (int i = 0; i < 6 * 120; i++) {
        float time = static_cast<float>(i) / 120.0f;
        rec.set_time_duration_secs("sim_time", time);
        float r_moon = time * 5.0f;
        float r_planet = time * 2.0f;

        rec.log(
            "planet_transforms",
            rerun::Transform3D::from_translation_rotation(
                {std::sin(r_planet) * d_planet, std::cos(r_planet) * d_planet, 0.0f},
                rerun::RotationAxisAngle{
                    {1.0f, 0.0f, 0.0f},
                    rerun::Angle::degrees(20.0f),
                }
            )
                .with_child_frame("planet_frame")
                .with_parent_frame("sun_frame")
        );
        rec.log(
            "moon_transforms",
            rerun::Transform3D::from_translation(
                {std::cos(r_moon) * d_moon, std::sin(r_moon) * d_moon, 0.0f}
            )
                .with_relation(rerun::components::TransformRelation::ChildFromParent)
                .with_child_frame("moon_frame")
                .with_parent_frame("planet_frame")
        );
    }
}
