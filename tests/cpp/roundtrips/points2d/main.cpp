#include <rerun.hpp>

#include <array>
#include <components/point2d.hpp>

int main(int argc, char** argv) {
    auto rr_stream = rr::RecordingStream{"c-example-app", "127.0.0.1:9876"};

    rr_stream.log_archetype(
        "points2d",
        rr::archetypes::Points2D({
                                     rr::datatypes::Point2D{1.0, 2.0},
                                     rr::datatypes::Point2D{3.0, 4.0},
                                 })
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({std::string("hello"), std::string("friend")})
            .with_draw_order(300.0)
            .with_class_ids({126, 127})
            .with_keypoint_ids({2, 3})
            .with_instance_keys({66, 666})
    );

    // Hack to establish 2d view bounds
    // TODO(andreas): There's no rect yet!
    // rr_stream.log_archetype("rect"
    //     .with_component(&[rr::datatypes::Rect2D(0.0, 0.0, 4.0, 6.0)])?
    //     .send(rec_stream)?;
}
