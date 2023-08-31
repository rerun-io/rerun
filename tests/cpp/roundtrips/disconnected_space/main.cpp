#include <rerun/archetypes/disconnected_space.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_roundtrip_disconnected_space");
    rec.save(argv[1]).throw_on_failure();

    rec.log("disconnected_space", rr::archetypes::DisconnectedSpace(true));
}
