#include <rerun/archetypes/pinhole.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_roundtrip_pinhole");
    rec.save(argv[1]).throw_on_failure();

    rec.log(
        "pinhole",
        rr::archetypes::Pinhole(
            rr::datatypes::Mat3x3({{3.0f, 0.0f, 1.5f}, {0.0f, 3.0f, 1.5f}, {0.0f, 0.0f, 1.0f}})
        ).with_resolution(rr::datatypes::Vec2D({3840.0f, 2160.0f}))
    );
}
