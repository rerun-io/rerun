#include <rerun.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rr_stream = rr::RecordingStream("rerun_example_roundtrip_text_log");
    rr_stream.save(argv[1]).throw_on_failure();
    rr_stream.log("log", rr::archetypes::TextLog("No level"));
    rr_stream.log("log", rr::archetypes::TextLog("INFO level").with_level("INFO"));
    rr_stream.log("log", rr::archetypes::TextLog("WILD level").with_level("WILD"));
}
