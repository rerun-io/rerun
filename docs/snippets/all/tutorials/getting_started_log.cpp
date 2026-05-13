#include <rerun.hpp>

#include <cmath>

int main(int argc, char* argv[]) {
    const auto rec =
        rerun::RecordingStream("rerun_example_getting_started", "run-1");
    rec.save("run-1.rrd").exit_on_failure();

    for (int t = 0; t < 10; ++t) {
        const auto tf = static_cast<double>(t);
        rec.set_time_duration_secs("t", tf);
        rec.log("/arm/shoulder", rerun::Scalars(std::sin(tf * 0.5)));
        rec.log("/arm/elbow", rerun::Scalars(std::cos(tf * 0.5)));
    }
}
