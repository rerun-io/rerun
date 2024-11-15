#include <rerun.hpp>

int main() {
    // Connect to the Rerun TCP server using the default address and
    // port: localhost:9876
    const auto rec = rerun::RecordingStream("rerun_example_native_sync");
    rec.connect_tcp().exit_on_failure();

    // Log data as usual, thereby pushing it into the TCP socket.
    while (true) {
        rec.log("log", rerun::TextLog("Logging things..."));
    }
}
