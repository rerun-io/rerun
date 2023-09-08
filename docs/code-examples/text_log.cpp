// Log a `TextLog`

#include <rerun.hpp>

#include <cmath>

namespace rr = rerun;
namespace rrd = rr::datatypes;

int main() {
    auto rr_stream = rr::RecordingStream("rerun_example_text_log");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    rr_stream.log(
        "log",
        rr::archetypes::TextLog("INFO level").with_level(rr::components::TextLogLevel::INFO)
    );
}
