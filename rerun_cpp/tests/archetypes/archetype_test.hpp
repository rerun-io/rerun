#pragma once

#include <arrow/buffer.h>
#include <catch2/catch_test_macros.hpp>

#include <rerun/component_batch.hpp>
#include <rerun/data_cell.hpp>

template <typename T>
void test_serialization_for_manual_and_builder(const T& from_manual, const T& from_builder) {
    THEN("convert to component lists") {
        std::vector<rerun::AnonymousComponentBatch> from_builder_lists =
            from_builder.as_component_batches();
        std::vector<rerun::AnonymousComponentBatch> from_manual_lists =
            from_manual.as_component_batches();

        REQUIRE(from_builder_lists.size() == from_manual_lists.size());

        AND_THEN("serializing each list succeeds") {
            std::vector<rerun::DataCell> from_builder_cells;
            std::vector<rerun::DataCell> from_manual_cells;
            for (size_t i = 0; i < from_builder_lists.size(); ++i) {
                auto from_builder_cell = from_builder_lists[i].to_data_cell();
                auto from_manual_cell = from_manual_lists[i].to_data_cell();

                REQUIRE(from_builder_cell.is_ok());
                REQUIRE(from_manual_cell.is_ok());

                from_builder_cells.push_back(from_builder_cell.value);
                from_manual_cells.push_back(from_manual_cell.value);
            }

            AND_THEN("the serialized data is the same") {
                for (size_t i = 0; i < from_builder_lists.size(); ++i) {
                    CHECK(
                        from_builder_cells[i].component_name == from_manual_cells[i].component_name
                    );
                    CHECK(from_builder_cells[i].buffer->Equals(*from_manual_cells[i].buffer));
                }
            }
        }
    }
}
