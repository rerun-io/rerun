// Disconnect two spaces.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("disconnected_space");
    rr_stream.connect("127.0.0.1:9876");

    // These two points can be projected into the same space..
    rr_stream.log(
        "world/room1/point",
        rr::archetypes::Points3D(rr::datatypes::Vec3D{0.0f, 0.0f, 0.0f})
    );
    rr_stream.log(
        "world/room2/point",
        rr::archetypes::Points3D(rr::datatypes::Vec3D{1.0f, 1.0f, 1.0f})
    );

    // ..but this one lives in a completely separate space!
    rr_stream.log("world/wormhole", rr::archetypes::DisconnectedSpace(true));
    rr_stream.log(
        "world/wormhole/point",
        rr::archetypes::Points3D(rr::datatypes::Vec3D{2.0f, 2.0f, 2.0f})
    );
}
