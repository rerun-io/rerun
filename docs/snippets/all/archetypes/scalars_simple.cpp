// Log a scalar over time.

#include <rerun.hpp>

#include <cmath>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_scalar");
    rec.spawn().exit_on_failure();

    // Log the data on a timeline called "step".
    for (int step = 0; step < 64; ++step) {
        rec.set_time_sequence("step", step);
        rec.log("scalar", rerun::Scalars(std::sin(static_cast<double>(step) / 10.0)));
    }
}
