#include <rerun/archetypes/boxes2d.hpp>
#include <rerun/archetypes/points2d.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_points2d");
    rec.save(argv[1]).exit_on_failure();

    rec.log(
        "points2d",
        rerun::archetypes::Points2D({{1.0, 2.0}, {3.0, 4.0}})
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_draw_order(300.0)
            .with_class_ids({126, 127})
            .with_keypoint_ids({2, 3})
    );

    // Hack to establish 2d view bounds
    rec.log(
        "rect",
        rerun::archetypes::Boxes2D::from_mins_and_sizes({{0.0f, 0.0f}}, {{4.0f, 6.0f}})
    );
}
