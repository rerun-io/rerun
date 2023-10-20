#include <catch2/catch_test_macros.hpp>

#include <rerun/datatypes/quaternion.hpp>
#include <rerun/datatypes/vec2d.hpp>
#include <rerun/datatypes/vec3d.hpp>
#include <rerun/datatypes/vec4d.hpp>

using namespace rerun::datatypes;

#define TEST_TAG "[vec_and_quaternion]"

void ctor_checks(const Vec2D& v2, const Vec3D& v3, const Vec4D& v4, const Quaternion& q) {
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
        Vec2D v2;
        Vec3D v3;
        Vec4D v4;
        Quaternion q;

        // Not initialized! Access is undefined behavior.
        // Suppress unused warnings.
        (void)(v2);
        (void)(v3);
        (void)(v4);
        (void)(q);
    }

    SECTION("Passing values to constructor") {
        Vec2D v2(1.0f, 2.0f);
        Vec3D v3(1.0f, 2.0f, 3.0f);
        Vec4D v4(1.0f, 2.0f, 3.0f, 4.0f);
        const auto q = Quaternion::from_xyzw(1.0f, 2.0f, 3.0f, 4.0f);

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via brace initialization") {
        Vec2D v2{1.0f, 2.0f};
        Vec3D v3{1.0f, 2.0f, 3.0f};
        Vec4D v4{1.0f, 2.0f, 3.0f, 4.0f};
        const auto q = Quaternion::from_xyzw(1.0f, 2.0f, 3.0f, 4.0f);

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via initializer list") {
        Vec2D v2({1.0f, 2.0f});
        Vec3D v3({1.0f, 2.0f, 3.0f});
        Vec4D v4({1.0f, 2.0f, 3.0f, 4.0f});
        const auto q = Quaternion::from_xyzw(1.0f, 2.0f, 3.0f, 4.0f);

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via std::array") {
        Vec2D v2(std::array<float, 2>{1.0f, 2.0f});
        Vec3D v3(std::array<float, 3>{1.0f, 2.0f, 3.0f});
        Vec4D v4(std::array<float, 4>{1.0f, 2.0f, 3.0f, 4.0f});
        Quaternion q(std::array<float, 4>{1.0f, 2.0f, 3.0f, 4.0f});

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via c-array") {
        float c_v2[2] = {1.0f, 2.0f};
        Vec2D v2(c_v2);

        float c_v3[3] = {1.0f, 2.0f, 3.0f};
        Vec3D v3(c_v3);

        float c_v4[4] = {1.0f, 2.0f, 3.0f, 4.0f};
        Vec4D v4(c_v4);

        float c_q[4] = {1.0f, 2.0f, 3.0f, 4.0f};
        const auto q = Quaternion::from_xyzw(c_q);

        ctor_checks(v2, v3, v4, q);
    }

    SECTION("Via float ptr") {
        std::array<float, 4> elements = {1.0f, 2.0f, 3.0f, 4.0f};

        Vec2D v2(elements.data());
        Vec3D v3(elements.data());
        Vec4D v4(elements.data());
        const auto q = Quaternion::from_xyzw(elements.data());

        ctor_checks(v2, v3, v4, q);
    }
}
