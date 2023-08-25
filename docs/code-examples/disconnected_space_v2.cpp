// Disconnect two spaces.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("rerun-example-disconnected_space");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    // These two points can be projected into the same space..
    rr_stream.log("world/room1/point", rr::Points3D(rr::datatypes::Vec3D{0.0f, 0.0f, 0.0f}));
    rr_stream.log("world/room2/point", rr::Points3D(rr::datatypes::Vec3D{1.0f, 1.0f, 1.0f}));

    // ..but this one lives in a completely separate space!
    rr_stream.log("world/wormhole", rr::DisconnectedSpace(true));
    rr_stream.log("world/wormhole/point", rr::Points3D(rr::datatypes::Vec3D{2.0f, 2.0f, 2.0f}));
}
