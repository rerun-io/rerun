// Showcase how to incrementally log data belonging to the same archetype, and re-use some or all
// of it across frames.

#include <rerun.hpp>

#include <algorithm>
#include <random>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_incremental_logging");
    rec.spawn().exit_on_failure();

    // TODO(#5264): just log one once clamp-to-edge semantics land.
    std::vector<rerun::Color> colors(10, rerun::Color(255, 0, 0));
    std::vector<rerun::Radius> radii(10, rerun::Radius(0.1));

    // Only log colors and radii once.
    rec.set_time_sequence("frame_nr", 0);
    rec.log("points", colors, radii);
    // Logging timelessly with `RecordingStream::log_timeless` would also work.
    // rec.log_timeless("points", colors, radii);

    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist_pos(-5.0f, 5.0f);

    // Then log only the points themselves each frame.
    //
    // They will automatically re-use the colors and radii logged at the beginning.
    for (int i = 0; i < 10; ++i) {
        rec.set_time_sequence("frame_nr", i);

        std::vector<rerun::Position3D> points(10);
        std::generate(points.begin(), points.end(), [&] {
            return rerun::Position3D(dist_pos(gen), dist_pos(gen), dist_pos(gen));
        });
        rec.log("points", rerun::Points3D(points));
    }
}
