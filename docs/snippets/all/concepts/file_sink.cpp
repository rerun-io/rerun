// Create and set a file sink.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_file_sink");

    rec.set_sinks(rerun::FileSink{"recording.rrd"}).exit_on_failure();
}
