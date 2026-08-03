#include <catch2/catch_test_macros.hpp>

#include <rerun/datatypes/mat3x3.hpp>
#include <rerun/datatypes/mat4x4.hpp>

using namespace rerun::datatypes;

#define TEST_TAG "[matrix_types]"

void ctor_checks(const Mat3x3& mat3x3) {
    for (size_t i = 0; i < 9; ++i) {
        CHECK(mat3x3.flat_columns[i] == static_cast<float>(i));
    }
}

TEST_CASE("Construct Mat3x3 in different ways", TEST_TAG) {
    SECTION("Default constructor") {
        Mat3x3 m3x3;

        // Not initialized! Access is undefined behavior.
        // Suppress unused warnings.
        (void)(m3x3);
    }

    SECTION("By passing vecs to constructor") {
        ctor_checks(
            Mat3x3({Vec3D{0.0f, 1.0f, 2.0f}, Vec3D{3.0f, 4.0f, 5.0f}, Vec3D{6.0f, 7.0f, 8.0f}})
        );
    }

    SECTION("Via c-array") {
        float carray[] = {0.0f, 1.0f, 2.0f, 3.0f, 4.0f, 5.0f, 6.0f, 7.0f, 8.0f};
        Mat3x3 m3x3(carray);
        ctor_checks(m3x3);
    }

    SECTION("Via float ptr") {
        std::array<float, 9> elements = {0.0f, 1.0f, 2.0f, 3.0f, 4.0f, 5.0f, 6.0f, 7.0f, 8.0f};
        Mat3x3 m3x3(elements.data());
        ctor_checks(m3x3);
    }
}

void ctor_checks(const Mat4x4& mat4x4) {
    for (size_t i = 0; i < 16; ++i) {
        CHECK(mat4x4.flat_columns[i] == static_cast<float>(i));
    }
}

TEST_CASE("Construct Mat4x4 in different ways", TEST_TAG) {
    SECTION("Default constructor") {
        Mat4x4 m4x4;

        // Not initialized! Access is undefined behavior.
        // Suppress unused warnings.
        (void)(m4x4);
    }

    SECTION("By passing vecs to constructor") {
        ctor_checks(Mat4x4({
            Vec4D{0.0f, 1.0f, 2.0f, 3.0f},
            Vec4D{4.0f, 5.0f, 6.0f, 7.0f},
            Vec4D{8.0f, 9.0f, 10.0f, 11.0f},
            Vec4D{12.0f, 13.0f, 14.0f, 15.0f},
        }));
    }

    SECTION("Via c-array") {
        float carray[] = {
            0.0f,
            1.0f,
            2.0f,
            3.0f,
            4.0f,
            5.0f,
            6.0f,
            7.0f,
            8.0f,
            9.0f,
            10.0f,
            11.0f,
            12.0f,
            13.0f,
            14.f,
            15.0f
        };
        Mat4x4 m4x4(carray);
        ctor_checks(m4x4);
    }

    SECTION("Via float ptr") {
        std::array<float, 16> elements = {
            0.0f,
            1.0f,
            2.0f,
            3.0f,
            4.0f,
            5.0f,
            6.0f,
            7.0f,
            8.0f,
            9.f,
            10.0f,
            11.0f,
            12.0f,
            13.0f,
            14.f,
            15.0f
        };
        Mat4x4 m4x4(elements.data());
        ctor_checks(m4x4);
    }
}
