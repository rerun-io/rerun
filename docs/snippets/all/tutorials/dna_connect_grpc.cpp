// The DNA-abacus example, connecting to a separately-running viewer over gRPC.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    // Connect to the viewer running at the default URL.
    const auto rec = rerun::RecordingStream("rerun_example_dna_abacus");
    rec.connect_grpc().exit_on_failure();

    // … log data as in the spawn-based example …
}
