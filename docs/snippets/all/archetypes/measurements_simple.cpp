// Log scalar measurements with variances over time.

#include <rerun.hpp>

#include <cmath>

int main(int argc, char* argv[]) {
    const auto rec =
        rerun::RecordingStream("rerun_example_measurements_simple");
    rec.spawn().exit_on_failure();

    // Two parallel pressure sensors (in Pa), each with slowly drifting variance.
    for (int step = 0; step < 64; ++step) {
        rec.set_time_sequence("step", step);
        const auto s = static_cast<double>(step);
        const std::array<double, 2> pressures = {
            101325.0 + 50.0 * std::sin(s / 10.0),
            101300.0 + 30.0 * std::cos(s / 8.0),
        };
        const std::array<double, 2> variances = {
            100.0 + 25.0 * std::sin(s / 7.0),
            80.0 + 15.0 * std::cos(s / 11.0),
        };
        rec.log(
            "pressure",
            rerun::Measurements(pressures).with_variances(variances).with_units(
                {"Pa"}
            )
        );
    }
}
