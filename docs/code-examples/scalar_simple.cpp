#include <rerun.hpp>

#include <cmath>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_scalar");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    for (int step = 0; step < 64; ++step) {
        rec.set_time_sequence("step", step);
        rec.log("scalar", rerun::TimeSeriesScalar(std::sin(static_cast<double>(step) / 10.0)));
    }
}
