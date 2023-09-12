#include <rerun/archetypes/boxes2d.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_roundtrip_box2d");
    rec.save(argv[1]).throw_on_failure();

    rec.log(
        "boxes2d",
        rr::archetypes::Boxes2D::from_half_sizes({{10.f, 9.f}, {5.f, -5.f}})
            .with_centers({{0.f, 0.f}, {-1.f, 1.f}})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_radii({0.1f, 1.0f})
            .with_draw_order(300.0)
            .with_class_ids({126, 127})
            .with_instance_keys({66, 666})
    );
}
