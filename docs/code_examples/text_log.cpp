// Log a `TextLog`

#include <rerun.hpp>

#include <cmath>

namespace rr = rerun;
namespace rrd = rr::datatypes;

int main() {
    auto rec = rr::RecordingStream("rerun_example_text_log");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log(
        "log",
        rr::archetypes::TextLog("Application started.")
            .with_level(rr::components::TextLogLevel::INFO)
    );
}
