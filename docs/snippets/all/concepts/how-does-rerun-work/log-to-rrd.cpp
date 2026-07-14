#include <rerun.hpp>

int main(int argc, char* argv[]) {
    // Open a local file handle to stream the data into.
    const auto rec = rerun::RecordingStream("rerun_example_log_to_rrd");
    rec.save("/tmp/my_recording.rrd").exit_on_failure();

    // Log data as usual, thereby writing it into the file.
    while (true) {
        rec.log("log", rerun::TextLog("Logging things…"));
    }
}
