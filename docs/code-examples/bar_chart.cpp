// Log a batch of 3D arrows.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_bar_chart");
    rec.connect().throw_on_failure();

    rec.log("bar_chart", rerun::BarChart::i64({8, 4, 0, 9, 1, 4, 1, 6, 9, 0}));
}
