#include <loguru.hpp>
#include <rerun.hpp>

#include <array>

namespace rr = rerun;
namespace rrc = rr::components;

int main(int argc, char** argv) {
    loguru::g_preamble_uptime = false;
    loguru::g_preamble_thread = false;
    loguru::init(argc, argv); // installs signal handlers

    LOG_F(INFO, "Rerun C++ SDK version: %s", rr::version_string());

    auto rr_stream = rr::RecordingStream("c-example-app");
    rr_stream.connect("127.0.0.1:9876");

    rr_stream.log_archetype(
        "3d/points",
        rr::archetypes::Points3D({rr::datatypes::Vec3D{1.0, 2.0, 3.0},
                                  rr::datatypes::Vec3D{4.0, 5.0, 6.0}})
            .with_radii({0.42, 0.43})
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({std::string("hello"), std::string("friend")})
            .with_class_ids({126, 127})
            .with_keypoint_ids({2, 3})
            .with_instance_keys({66, 666})
    );

    rrc::Label c_style_array[3] = {rrc::Label("hello"), rrc::Label("friend"), rrc::Label("yo")};
    rr_stream.log_components(
        "2d/points",
        std::vector{
            rrc::Point2D(rr::datatypes::Vec2D{0.0, 0.0}),
            rrc::Point2D(rr::datatypes::Vec2D{1.0, 3.0}),
            rrc::Point2D(rr::datatypes::Vec2D{5.0, 5.0})},
        std::array{rrc::Color(0xFF0000FF), rrc::Color(0x00FF00FF), rrc::Color(0x0000FFFF)},
        c_style_array
    );

    // Test some type instantiation
    auto tls = rr::datatypes::TranslationRotationScale3D{};
    tls.translation = {1.0, 2.0, 3.0};
    rr::datatypes::Transform3D t = std::move(tls);
}
