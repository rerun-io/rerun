#include <rerun/archetypes/arrows3d.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_arrows3d");
    rec.save(argv[1]).exit_on_failure();

    rec.log(
        "arrows3d",
        rerun::archetypes::Arrows3D::from_vectors({{4.0f, 5.0f, 6.0f}, {40.0f, 50.0f, 60.0f}})
            .with_origins({{1.0f, 2.0f, 3.0f}, {10.0f, 20.0f, 30.0f}})
            .with_radii({0.1f, 1.0f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_class_ids({126, 127})
    );
}
