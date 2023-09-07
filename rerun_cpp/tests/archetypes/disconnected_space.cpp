#include "archetype_test.hpp"

#include <rerun/archetypes/disconnected_space.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[disconnected_space][archetypes]"

SCENARIO("disconnected_space archetype can be serialized" TEST_TAG) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder = DisconnectedSpace(true);

        THEN("serialization succeeds") {
            for (auto& list : from_builder.as_component_lists()) {
                CHECK(list.to_data_cell().is_ok());
            }
        }
    }
}
