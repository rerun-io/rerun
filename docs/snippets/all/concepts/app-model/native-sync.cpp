#include <rerun.hpp>

int main() {
    // Connect to the Rerun gRPC server using the default address and
    // port: localhost:9876
    const auto rec = rerun::RecordingStream("rerun_example_native_sync");
    rec.connect_grpc().exit_on_failure();

    // Log data as usual, thereby pushing it into the gRPC connection.
    while (true) {
        rec.log("log", rerun::TextLog("Logging things..."));
    }
}
