#include "archetype_test.hpp"

#include <rerun/archetypes/view_coordinates.hpp>
#include <rerun/components/view_coordinates.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[view_coordinates][archetypes]"

SCENARIO(
    "ViewCoordinates archetype can be serialized with the same result whether from builder, static "
    "const, or manually.",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder = ViewCoordinates(
            rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        );

        ViewCoordinates from_manual;
        from_manual.coordinates = {
            rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        };

        test_serialization_for_manual_and_builder(from_manual, from_builder);
    }

    GIVEN("Constructed from builder and static") {
        auto from_builder = ViewCoordinates(
            rerun::components::ViewCoordinates::Right,
            rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        );

        test_serialization_for_manual_and_builder(ViewCoordinates::RDF, from_builder);
    }
}
