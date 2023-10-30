#include <rerun/archetypes/disconnected_space.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_disconnected_space");
    rec.save(argv[1]).throw_on_failure();

    rec.log("disconnected_space", rerun::archetypes::DisconnectedSpace(true));
}
