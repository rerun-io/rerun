#include <rerun.hpp>

#include <array>
#include <components/point3d.hpp>

int main(int argc, char** argv) {
    auto rr_stream = rr::RecordingStream{"c-example-app", "127.0.0.1:9876"};

    rr_stream.log_archetype(
        "points3d",
        rr::archetypes::Points3D({
                                     rr::datatypes::Point3D{1.0, 2.0, 3.0},
                                     rr::datatypes::Point3D{4.0, 5.0, 6.0},
                                 })
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({std::string("hello"), std::string("friend")})
            .with_class_ids({126, 127})
            .with_keypoint_ids({2, 3})
            .with_instance_keys({66, 666})
    );
}
