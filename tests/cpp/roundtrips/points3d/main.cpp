#include <rerun/archetypes/points3d.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec_stream = rr::RecordingStream("roundtrip_points3d");
    rec_stream.save(argv[1]);

    rec_stream.log_archetype(
        "points3d",
        rr::archetypes::Points3D({rr::datatypes::Vec3D{1.0, 2.0, 3.0},
                                  rr::datatypes::Vec3D{4.0, 5.0, 6.0}})
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({std::string("hello"), std::string("friend")})
            .with_class_ids({126, 127})
            .with_keypoint_ids({2, 3})
            .with_instance_keys({66, 666})
    );
}
