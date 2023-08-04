#include <catch2/catch_test_macros.hpp>

#include <rerun/components/color.hpp>

using namespace rerun::components;

#define TEST_TAG "[color]"

TEST_CASE("Construct Color in different ways", TEST_TAG) {
    SECTION("Default constructor") {
        Color c;

        // Not initialized! Access is undefined behavior.
        // Suppress unused warning.
        (void)(c);
    }

    SECTION("Passing RGBA to constructor") {
        Color c(1, 2, 3, 4);
        CHECK(c.r() == 1);
        CHECK(c.g() == 2);
        CHECK(c.b() == 3);
        CHECK(c.a() == 4);
    }

    SECTION("Passing RGB to constructor") {
        Color c(1, 2, 3);
        CHECK(c.r() == 1);
        CHECK(c.g() == 2);
        CHECK(c.b() == 3);
        CHECK(c.a() == 255);
    }

    SECTION("Passing RGBA to constructor via initializer list") {
        Color c({1, 2, 3, 4});
        CHECK(c.r() == 1);
        CHECK(c.g() == 2);
        CHECK(c.b() == 3);
        CHECK(c.a() == 4);
    }

    SECTION("Passing RGB to constructor via initializer list") {
        Color c({1, 2, 3});
        CHECK(c.r() == 1);
        CHECK(c.g() == 2);
        CHECK(c.b() == 3);
        CHECK(c.a() == 255);
    }

    SECTION("Passing RGBA to constructor via c array") {
        uint8_t rgba[4] = {1, 2, 3, 4};
        Color c(rgba);
        CHECK(c.r() == 1);
        CHECK(c.g() == 2);
        CHECK(c.b() == 3);
        CHECK(c.a() == 4);
    }

    SECTION("Passing RGB to constructor via c array") {
        uint8_t rgb[3] = {1, 2, 3};
        Color c(rgb);
        CHECK(c.r() == 1);
        CHECK(c.g() == 2);
        CHECK(c.b() == 3);
        CHECK(c.a() == 255);
    }
}
