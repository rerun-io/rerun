// Log a scalar over time.

#include <rerun.hpp>

#include <cmath>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_scalar");
    rec.connect().throw_on_failure();

    for (int step = 0; step < 64; ++step) {
        rec.set_time_sequence("step", step);
        rec.log("scalar", rerun::TimeSeriesScalar(std::sin(static_cast<double>(step) / 10.0)));
    }
}
