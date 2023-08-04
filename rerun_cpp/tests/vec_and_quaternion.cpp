#include <catch2/catch_test_macros.hpp>

#include <rerun/datatypes/quaternion.hpp>
#include <rerun/datatypes/vec2d.hpp>
#include <rerun/datatypes/vec3d.hpp>
#include <rerun/datatypes/vec4d.hpp>

#include <array>

namespace rr = rerun;

#define TEST_TAG "[vec_and_quaternion]"

void ctor_checks(
    const rr::datatypes::Vec2D& v2, const rr::datatypes::Vec3D& v3, const rr::datatypes::Vec4D& v4,
    const rr::datatypes::Quaternion& q
) {
    CHECK(v2.x() == 1.0);
    CHECK(v2.y() == 2.0);

    CHECK(v3.x() == 1.0);
    CHECK(v3.y() == 2.0);
    CHECK(v3.z() == 3.0);

    CHECK(v4.x() == 1.0);
    CHECK(v4.y() == 2.0);
    CHECK(v4.z() == 3.0);
    CHECK(v4.w() == 4.0);

    CHECK(q.x() == 1.0);
    CHECK(q.y() == 2.0);
    CHECK(q.z() == 3.0);
    CHECK(q.w() == 4.0);
}

TEST_CASE("Construct VecND in different ways", TEST_TAG) {
    SECTION("Default constructor") {
        rr::datatypes::Vec2D v2;
        rr::datatypes::Vec3D v3;
        rr::datatypes::Vec4D v4;
        rr::datatypes::Quaternion q;

        // Not initialized! Access is undefined behavior.
        // Suppress unused warnings.
        (void)(v2);
        (void)(v3);
        (void)(v4);
        (void)(q);
    }

    SECTION("Passing values to constructor") {
        rr::datatypes::Vec2D v2(1.0f, 2.0f);
        rr::datatypes::Vec3D v3(1.0f, 2.0f, 3.0f);
        rr::datatypes::Vec4D v4(1.0f, 2.0f, 3.0f, 4.0f);
        rr::datatypes::Quaternion q(1.0f, 2.0f, 3.0f, 4.0f);

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via brace initialization") {
        rr::datatypes::Vec2D v2{1.0f, 2.0f};
        rr::datatypes::Vec3D v3{1.0f, 2.0f, 3.0f};
        rr::datatypes::Vec4D v4{1.0f, 2.0f, 3.0f, 4.0f};
        rr::datatypes::Quaternion q{1.0f, 2.0f, 3.0f, 4.0f};

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via initializer list") {
        rr::datatypes::Vec2D v2({1.0f, 2.0f});
        rr::datatypes::Vec3D v3({1.0f, 2.0f, 3.0f});
        rr::datatypes::Vec4D v4({1.0f, 2.0f, 3.0f, 4.0f});
        rr::datatypes::Quaternion q({1.0f, 2.0f, 3.0f, 4.0f});

        ctor_checks(v2, v3, v4, q);
    }

    // Dropped this since providing an std::array version makes the initializer list version
    // ambiguous.
    // SECTION("Via std::array") {
    //     rr::datatypes::Vec2D v2(std::array<float, 2>{1.0f, 2.0f});
    //     rr::datatypes::Vec3D v3(std::array<float, 3>{1.0f, 2.0f, 3.0f});
    //     rr::datatypes::Vec4D v4(std::array<float, 4>{1.0f, 2.0f, 3.0f, 4.0f});
    //     rr::datatypes::Quaternion q(std::array<float, 4>{1.0f, 2.0f, 3.0f, 4.0f});

    //     ctor_checks(v2, v3, v4, q);
    // }

    SECTION("Via c-array") {
        float c_v2[2] = {1.0f, 2.0f};
        rr::datatypes::Vec2D v2(c_v2);

        float c_v3[3] = {1.0f, 2.0f, 3.0f};
        rr::datatypes::Vec3D v3(c_v3);

        float c_v4[4] = {1.0f, 2.0f, 3.0f, 4.0f};
        rr::datatypes::Vec4D v4(c_v4);

        float c_q[4] = {1.0f, 2.0f, 3.0f, 4.0f};
        rr::datatypes::Quaternion q(c_q);

        ctor_checks(v2, v3, v4, q);
    }
}
