// Log a scalar over time.

#include <rerun.hpp>

#include <cmath>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_scalar");
    rec.spawn().exit_on_failure();

    // Set up plot styling: Logged timeless since it never changes and affects all timelines.
    rec.log_timeless("scalar", rerun::SeriesPoint());

    // Log the data on a timeline called "step".
    for (int step = 0; step < 64; ++step) {
        rec.set_time_sequence("step", step);
        rec.log("scalar", rerun::Scalar(std::sin(static_cast<double>(step) / 10.0)));
    }
}
