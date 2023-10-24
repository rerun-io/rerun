#include <rerun.hpp>

#include <array>

namespace rrc = rerun::components;

int main() {
    auto rec = rerun::RecordingStream("rerun_example_cpp_app");
    rec.connect().throw_on_failure();

    // Log points with the archetype api - this is the preferred way of logging.
    rec.log(
        "3d/points",
        rerun::Points3D({{1.0f, 2.0f, 3.0f}, {4.0f, 5.0f, 6.0f}})
            .with_radii({0.42f, 0.43f})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_class_ids({126, 127})
            .with_keypoint_ids({2, 3})
            .with_instance_keys({66, 666})
    );

    // Log points with the components api - this is the advanced way of logging components in a
    // fine-grained matter. It supports passing various types of containers.
    rrc::Text c_style_array[3] = {rrc::Text("hello"), rrc::Text("friend"), rrc::Text("yo")};
    rec.log(
        "2d/points",
        std::vector{
            rrc::Position2D(0.0f, 0.0f),
            rrc::Position2D(1.0f, 3.0f),
            rrc::Position2D(5.0f, 5.0f),
        },
        std::array{rrc::Color(0xFF0000FF), rrc::Color(0x00FF00FF), rrc::Color(0x0000FFFF)},
        c_style_array
    );

    // Test some type instantiation
    auto tls = rerun::datatypes::TranslationRotationScale3D{};
    tls.translation = {1.0, 2.0, 3.0};
    rerun::datatypes::Transform3D t = std::move(tls);
}
