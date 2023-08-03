#include <archetypes/disconnected_space.hpp> // TODO(andreas): Should be rerun/ prefixed.
#include <recording_stream.hpp>

int main(int argc, char** argv) {
    auto rec_stream = rr::RecordingStream("roundtrip_disconnected_space");
    rec_stream.save(argv[1]);

    rec_stream.log_archetype("disconnected_space", rr::archetypes::DisconnectedSpace(true));
}
