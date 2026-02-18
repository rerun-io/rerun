// Log a scalar over time.

#include <rerun.hpp>

#include <cmath>

constexpr float TAU = 6.28318530717958647692528676655900577f;

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_scalar_multiple_plots");
    rec.spawn().exit_on_failure();

    int64_t lcg_state = 0;

    // Set up plot styling:
    // They are logged static as they don't change over time and apply to all timelines.
    // Log two lines series under a shared root so that they show in the same plot by default.
    rec.log_static(
        "trig/sin",
        rerun::SeriesLines().with_colors(rerun::Rgba32{255, 0, 0}).with_names("sin(0.01t)")
    );
    rec.log_static(
        "trig/cos",
        rerun::SeriesLines().with_colors(rerun::Rgba32{0, 255, 0}).with_names("cos(0.01t)")
    );

    // NOTE: `SeriesLines` and `SeriesPoints` can both be logged without any associated data
    //       (all fields are optional). In `v0.24` we removed indicators, which now results in
    //       no data logged at all, when no fields are specified. Therefore we log a circle shape
    //       here. More information: https://github.com/rerun-io/rerun/issues/10512

    // Log scattered points under a different root so that they show in a different plot by default.
    rec.log_static(
        "scatter/lcg",
        rerun::SeriesPoints().with_markers(rerun::components::MarkerShape::Circle)
    );

    // Log the data on a timeline called "step".
    for (int t = 0; t < static_cast<int>(TAU * 2.0 * 100.0); ++t) {
        rec.set_time_sequence("step", t);

        rec.log("trig/sin", rerun::Scalars(sin(static_cast<double>(t) / 100.0)));
        rec.log("trig/cos", rerun::Scalars(cos(static_cast<double>(t) / 100.0)));

        lcg_state =
            (1140671485 * lcg_state + 128201163) % 16777216; // simple linear congruency generator
        rec.log("scatter/lcg", rerun::Scalars(static_cast<double>(lcg_state)));
    }
}
