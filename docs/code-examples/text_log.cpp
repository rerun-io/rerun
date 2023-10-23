// Log a `TextLog`

#include <rerun.hpp>

#include <cmath>

namespace rrd = rerun::datatypes;

int main() {
    auto rec = rerun::RecordingStream("rerun_example_text_log");
    rec.connect().throw_on_failure();

    rec.log(
        "log",
        rerun::archetypes::TextLog("Application started.")
            .with_level(rerun::components::TextLogLevel::INFO)
    );
}
