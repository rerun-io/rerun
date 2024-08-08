// Use the log APIs to log scalars over time.

#include <rerun.hpp>

#include <cmath>

const size_t NUM_STEPS = 100000;
const double COEFF = 10.0 / static_cast<double>(NUM_STEPS);

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_log_rows");
    rec.to_stdout().exit_on_failure();

    // Log the data on a timeline called "step".
    for (int step = 0; step < NUM_STEPS; ++step) {
        // Set the `step` timeline in the logging context to the current time.
        rec.set_time_sequence("step", step);

        // Log a new row containing a single scalar.
        // This will inherit from the logging context, and thus be logged at the current `step`.
        rec.log("scalar", rerun::Scalar(std::sin(static_cast<double>(step) * COEFF)));
    }
}

