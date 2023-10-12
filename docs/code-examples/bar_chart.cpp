// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_bar_chart");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("bar_chart", rr::BarChart::i64({8, 4, 0, 9, 1, 4, 1, 6, 9, 0}));
}
