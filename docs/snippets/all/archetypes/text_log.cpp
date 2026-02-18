// Log a `TextLog`

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_text_log");
    rec.spawn().exit_on_failure();

    rec.log("log", rerun::TextLog("Application started.").with_level(rerun::TextLogLevel::Info));
}
