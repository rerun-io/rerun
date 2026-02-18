#include <rerun.hpp>

int main(int argc, char* argv[]) {
    // Connect to the Rerun gRPC server using the default address and
    // port: localhost:9876
    const auto rec = rerun::RecordingStream("rerun_example_log_to_grpc");
    rec.connect_grpc().exit_on_failure();

    // Log data as usual, thereby pushing it into the gRPC connection.
    while (true) {
        rec.log("log", rerun::TextLog("Logging things…"));
    }
}
