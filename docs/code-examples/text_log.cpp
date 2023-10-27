// Log a `TextLog`

#include <rerun.hpp>

#include <cmath>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_text_log");
    rec.spawn().throw_on_failure();

    rec.log("log", rerun::TextLog("Application started.").with_level(rerun::TextLogLevel::INFO));
}
