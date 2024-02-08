#include <rerun/archetypes/line_strips3d.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_line_strips3d");
    rec.save(argv[1]).exit_on_failure();

    rec.log(
        "line_strips3d",
        rerun::archetypes::LineStrips3D(
            {rerun::components::LineStrip3D({{0.f, 0.f, 0.f}, {2.f, 1.f, -1.f}}),
             rerun::components::LineStrip3D({{4.f, -1.f, 3.f}, {6.f, 0.f, 1.5f}})}
        )
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_class_ids({126, 127})
            .with_instance_keys({66, 666})
    );
}
