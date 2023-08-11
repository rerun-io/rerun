#pragma once

#include <catch2/catch_test_macros.hpp>

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#include <arrow/buffer.h>
#include <arrow/result.h>
#pragma GCC diagnostic pop

template <typename T>
void test_serialization_for_manual_and_builder(const T& from_manual, const T& from_builder) {
    THEN("serialization succeeds") {
        auto from_builder_serialized = from_builder.to_data_cells();
        REQUIRE(from_builder_serialized.ok());

        auto from_manual_serialized = from_manual.to_data_cells();
        REQUIRE(from_manual_serialized.ok());

        AND_THEN("the serialized data is the same") {
            auto from_builder_cells = from_builder_serialized.ValueOrDie();
            auto from_manual_cells = from_manual_serialized.ValueOrDie();

            CHECK(from_builder_cells.size() == from_manual_cells.size());
            for (size_t i = 0; i < from_builder_cells.size(); ++i) {
                CHECK(from_builder_cells[i].component_name == from_manual_cells[i].component_name);
                CHECK(from_builder_cells[i].buffer->Equals(*from_manual_cells[i].buffer));
            }
        }
    }
}
