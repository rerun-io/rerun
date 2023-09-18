#include <rerun/archetypes/boxes2d.hpp>
#include <rerun/archetypes/line_strips2d.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec = rr::RecordingStream("rerun_example_roundtrip_line_strip2d");
    rec.save(argv[1]).throw_on_failure();

    rec.log(
        "line_strips3d",
        rr::archetypes::LineStrips2D({rr::components::LineStrip2D({{0.f, 0.f}, {2.f, 1.f}}),
                                      rr::components::LineStrip2D({{4.f, -1.f}, {6.f, 0.f}})})
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_draw_order(300.0)
            .with_class_ids({126, 127})
            .with_instance_keys({66, 666})
    );

    // Hack to establish 2d view bounds
    rec.log(
        "rect",
        rr::archetypes::Boxes2D::from_mins_and_sizes({{-10.0f, -10.0f}}, {{20.0f, 20.0f}})
    );
}
