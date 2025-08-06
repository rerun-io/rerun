#include <catch2/catch_test_macros.hpp>

#include <rerun.hpp>

#include "error_check.hpp"

#define TEST_TAG "[log_empty][archetypes]"

// Regression test for #3840
SCENARIO("Log empty data", TEST_TAG) {
    rerun::RecordingStream stream("empty archetype");

    SECTION("Using an existing archetype") {
        check_logged_error([&] {
            stream.log("empty", rerun::Points3D(std::vector<rerun::Position3D>{}));
        });
    }
    SECTION("Using an empty component batch") {
        check_logged_error([&] {
            stream.log(
                "empty",
                rerun::ComponentBatch::empty<rerun::Position3D>(
                    rerun::Points3D::Descriptor_positions
                )
            );
        });
    }
}
