// Disconnect two spaces.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_disconnected_space");
    rec.spawn().exit_on_failure();

    // These two points can be projected into the same space..
    rec.log("world/room1/point", rerun::Points3D({{0.0f, 0.0f, 0.0f}}));
    rec.log("world/room2/point", rerun::Points3D({{1.0f, 1.0f, 1.0f}}));

    // ..but this one lives in a completely separate space!
    rec.log("world/wormhole", rerun::DisconnectedSpace(true));
    rec.log("world/wormhole/point", rerun::Points3D({{2.0f, 2.0f, 2.0f}}));
}
