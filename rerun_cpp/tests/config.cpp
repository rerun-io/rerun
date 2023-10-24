#include <catch2/catch_test_macros.hpp>

#include <rerun.hpp>

#define TEST_TAG "[set_enabled]"

SCENARIO("Rerun can be disabled", TEST_TAG) {
    GIVEN("The initial state") {
        THEN("The default value of enabled is true") {
            CHECK(rerun::is_enabled());
        }
    }

    GIVEN("Logging has been disabled") {
        rerun::set_enabled(false);

        THEN("is_enabled returns false") {
            CHECK_FALSE(rerun::is_enabled());
        }
    }
}
