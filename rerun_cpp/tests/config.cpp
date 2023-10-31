#include <catch2/catch_test_macros.hpp>

#include <rerun.hpp>

#define TEST_TAG "[set_enabled]"

SCENARIO("Rerun default_enabled can be configured", TEST_TAG) {
    GIVEN("The initial state") {
        THEN("The default value of default_enabled is true") {
            CHECK(rerun::is_default_enabled());
        }
    }

    GIVEN("Logging has been disabled") {
        rerun::set_default_enabled(false);

        THEN("default_enabled returns false") {
            CHECK_FALSE(rerun::is_default_enabled());
        }
    }

    // NOTE: This needs to go last or else we end up globally disabling
    // default recordings, which breaks other tests.
    GIVEN("Logging has been enabled") {
        rerun::set_default_enabled(true);

        THEN("default_enabled returns true") {
            CHECK(rerun::is_default_enabled());
        }
    }
}
