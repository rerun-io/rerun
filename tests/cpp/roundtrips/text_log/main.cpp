#include <rerun.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_text_log");
    rec.save(argv[1]).exit_on_failure();
    rec.log("log", rerun::archetypes::TextLog("No level"));
    rec.log(
        "log",
        rerun::archetypes::TextLog("INFO level").with_level(rerun::components::TextLogLevel::Info)
    );
    rec.log("log", rerun::archetypes::TextLog("WILD level").with_level("WILD"));
}
