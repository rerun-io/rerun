#include <rerun/archetypes/arrows3d.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec_stream = rr::RecordingStream("roundtrip_arrows3d");
    rec_stream.save(argv[1]);

    rec_stream.log(
        "arrows3d",
        rr::archetypes::Arrows3D({{4.0f, 5.0f, 6.0f}, {40.0f, 50.0f, 60.0f}})
            .with_origins({{1.0f, 2.0f, 3.0f}, {10.0f, 20.0f, 30.0f}})
            .with_radii({0.1f, 1.0f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_class_ids({126, 127})
            .with_instance_keys({66, 666})
    );
}
