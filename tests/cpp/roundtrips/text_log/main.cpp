#include <rerun.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_roundtrip_text_log");
    rec.save(argv[1]).throw_on_failure();
    rec.log("log", rr::archetypes::TextLog("No level"));
    rec.log(
        "log",
        rr::archetypes::TextLog("INFO level").with_level(rr::components::TextLogLevel::INFO)
    );
    rec.log("log", rr::archetypes::TextLog("WILD level").with_level("WILD"));
}
