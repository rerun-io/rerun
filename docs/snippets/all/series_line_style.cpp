// Log a scalar over time.

#include <rerun.hpp>

#include <cmath>

constexpr float TAU = 6.28318530717958647692528676655900577f;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_series_line_style");
    rec.spawn().exit_on_failure();

    // Set up plot styling:
    // They are logged timeless as they don't change over time and apply to all timelines.
    // Log two lines series under a shared root so that they show in the same plot by default.
    rec.log_timeless(
        "trig/sin",
        rerun::SeriesLine().with_color({255, 0, 0}).with_name("sin(0.01t)").with_width(2)
    );
    rec.log_timeless(
        "trig/cos",
        rerun::SeriesLine().with_color({0, 255, 0}).with_name("cos(0.01t)").with_width(4)
    );

    // Log the data on a timeline called "step".
    for (int t = 0; t < static_cast<int>(TAU * 2.0 * 100.0); ++t) {
        rec.set_time_sequence("step", t);

        rec.log("trig/sin", rerun::Scalar(sin(static_cast<double>(t) / 100.0)));
        rec.log("trig/cos", rerun::Scalar(cos(static_cast<double>(t) / 100.0f)));
    }
}
