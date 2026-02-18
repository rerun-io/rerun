// Create and set a GRPC sink.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_grpc_sink");

    // The default URL is `rerun+http://127.0.0.1:9876/proxy`
    // This can be used to connect to a viewer on a different machine
    rec.set_sinks(rerun::GrpcSink{"rerun+http://127.0.0.1:9876/proxy"}).exit_on_failure();
}
