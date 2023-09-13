#include "archetype_test.hpp"

#include <rerun/archetypes/clear.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[clear][archetypes]"

SCENARIO("clear archetype can be serialized" TEST_TAG) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder = Clear(true);

        THEN("serialization succeeds") {
            for (auto& list : from_builder.as_component_batches()) {
                CHECK(list.to_data_cell().is_ok());
            }
        }
    }
}
