// Create and set a FileSink

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_file_sink");

    rec.set_sinks(rerun::FileSink{"recording.rrd"});
}
