//! Update a scalar over time.
//!
//! See also the `scalar_column_updates` example, which achieves the same thing in a single operation.

#include <cmath>

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_scalar_row_updates");
    rec.spawn().exit_on_failure();

    for (int step = 0; step < 64; ++step) {
        rec.set_time_sequence("step", step);
        rec.log("scalars", rerun::Scalars(sin(static_cast<double>(step) / 10.0)));
    }
}
