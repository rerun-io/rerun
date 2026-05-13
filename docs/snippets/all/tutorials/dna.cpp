// The DNA-abacus example from the Log and Ingest tutorial.

// region: imports
#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

#include <algorithm> // std::generate
#include <random>
#include <vector>

// endregion: imports

int main(int argc, char* argv[]) {
    using namespace rerun::demo;
    using namespace std::chrono_literals;

    // region: init
    const auto rec = rerun::RecordingStream("rerun_example_dna_abacus");
    rec.spawn().exit_on_failure();
    // endregion: init

    // The fix for the latest-at lesson — see "Latest-at semantics" in the tutorial.
    // region: latest_at_fix
    rec.set_time_duration("stable_time", 0s);
    // endregion: latest_at_fix

    constexpr size_t NUM_POINTS = 100;

    // region: first_points
    std::vector<rerun::Position3D> points1, points2;
    std::vector<rerun::Color> colors1, colors2;
    color_spiral(NUM_POINTS, 2.0f, 0.02f, 0.0f, 0.1f, points1, colors1);
    color_spiral(NUM_POINTS, 2.0f, 0.02f, TAU * 0.5f, 0.1f, points2, colors2);

    rec.log(
        "dna/structure/left",
        rerun::Points3D(points1).with_colors(colors1).with_radii({0.08f})
    );
    rec.log(
        "dna/structure/right",
        rerun::Points3D(points2).with_colors(colors2).with_radii({0.08f})
    );
    // endregion: first_points

    // region: scaffolding
    std::vector<rerun::LineStrip3D> lines;
    for (size_t i = 0; i < points1.size(); ++i) {
        lines.emplace_back(rerun::LineStrip3D({points1[i].xyz, points2[i].xyz})
        );
    }

    rec.log(
        "dna/structure/scaffolding",
        rerun::LineStrips3D(lines).with_colors(rerun::Color(128, 128, 128))
    );
    // endregion: scaffolding

    // region: beads
    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist(0.0f, 1.0f);
    std::vector<float> offsets(NUM_POINTS);
    std::generate(offsets.begin(), offsets.end(), [&] { return dist(gen); });

    std::vector<rerun::Position3D> beads_positions(lines.size());
    std::vector<rerun::Color> beads_colors(lines.size());

    for (size_t i = 0; i < lines.size(); ++i) {
        auto c =
            static_cast<uint8_t>(bounce_lerp(80.0f, 230.0f, offsets[i] * 2.0f));
        beads_positions[i] = rerun::Position3D(
            bounce_lerp(
                lines[i].points[0].x(),
                lines[i].points[1].x(),
                offsets[i]
            ),
            bounce_lerp(
                lines[i].points[0].y(),
                lines[i].points[1].y(),
                offsets[i]
            ),
            bounce_lerp(
                lines[i].points[0].z(),
                lines[i].points[1].z(),
                offsets[i]
            )
        );
        beads_colors[i] = rerun::Color(c, c, c);
    }

    rec.log(
        "dna/structure/scaffolding/beads",
        rerun::Points3D(beads_positions)
            .with_colors(beads_colors)
            .with_radii({0.06f})
    );
    // endregion: beads

    // region: time_loop
    for (int t = 0; t < 400; t++) {
        auto time = std::chrono::duration<float>(t) * 0.01f;

        rec.set_time_duration("stable_time", time);

        for (size_t i = 0; i < lines.size(); ++i) {
            float time_offset = time.count() + offsets[i];
            auto c = static_cast<uint8_t>(
                bounce_lerp(80.0f, 230.0f, time_offset * 2.0f)
            );

            beads_positions[i] = rerun::Position3D(
                bounce_lerp(
                    lines[i].points[0].x(),
                    lines[i].points[1].x(),
                    time_offset
                ),
                bounce_lerp(
                    lines[i].points[0].y(),
                    lines[i].points[1].y(),
                    time_offset
                ),
                bounce_lerp(
                    lines[i].points[0].z(),
                    lines[i].points[1].z(),
                    time_offset
                )
            );
            beads_colors[i] = rerun::Color(c, c, c);
        }

        rec.log(
            "dna/structure/scaffolding/beads",
            rerun::Points3D(beads_positions)
                .with_colors(beads_colors)
                .with_radii({0.06f})
        );
    }
    // endregion: time_loop

    // region: transform_loop
    for (int t = 0; t < 400; t++) {
        auto time = std::chrono::duration<float>(t) * 0.01f;

        rec.set_time_duration("stable_time", time);

        rec.log(
            "dna/structure",
            rerun::archetypes::Transform3D(rerun::RotationAxisAngle(
                {0.0f, 0.0f, 1.0f},
                rerun::Angle::radians(time.count() / 4.0f * TAU)
            ))
        );
    }
    // endregion: transform_loop
}
