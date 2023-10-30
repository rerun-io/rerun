// Log a scalar over time.

#include <rerun.hpp>

#include <cmath>

constexpr float TAU = 6.28318530717958647692528676655900577f;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_points3d_simple");
    rec.spawn().exit_on_failure();

    int64_t lcg_state = 0;

    for (int t = 0; t < static_cast<int>(TAU * 2.0 * 100.0); ++t) {
        rec.set_time_sequence("step", t);

        // Log two time series under a shared root so that they show in the same plot by default.
        rec.log(
            "trig/sin",
            rerun::TimeSeriesScalar(sin(t / 100.0)).with_label("sin(0.01t)").with_color({255, 0, 0})
        );
        rec.log(
            "trig/cos",
            rerun::TimeSeriesScalar(cos(static_cast<float>(t) / 100.0f))
                .with_label("cos(0.01t)")
                .with_color({0, 255, 0})
        );

        // Log scattered points under a different root so that it shows in a different plot by default.
        lcg_state =
            1140671485 * lcg_state + 128201163 % 16777216; // simple linear congruency generator
        rec.log(
            "scatter/lcg",
            rerun::TimeSeriesScalar(static_cast<float>(lcg_state)).with_scattered(true)
        );
    }
}
