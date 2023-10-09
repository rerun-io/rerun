#pragma once

#include <arrow/buffer.h>
#include <catch2/catch_test_macros.hpp>

#include <rerun/component_batch.hpp>
#include <rerun/data_cell.hpp>

template <typename T>
void test_serialization_for_manual_and_builder(const T& from_manual, const T& from_builder) {
    THEN("convert to component lists") {
        auto from_builder_serialized_result = from_builder.serialize();
        auto from_manual_serialized_result = from_manual.serialize();

        AND_THEN("serializing each list succeeds") {
            REQUIRE(from_builder_serialized_result.is_ok());
            REQUIRE(from_manual_serialized_result.is_ok());

            const auto& from_builder_serialized = from_builder_serialized_result.value;
            const auto& from_manual_serialized = from_manual_serialized_result.value;
            REQUIRE(from_builder_serialized.size() == from_manual_serialized.size());

            AND_THEN("the serialized data is the same") {
                for (size_t i = 0; i < from_builder_serialized.size(); ++i) {
                    CHECK(
                        from_builder_serialized[i].num_instances ==
                        from_manual_serialized[i].num_instances
                    );
                    CHECK(
                        from_builder_serialized[i].data_cell.component_name ==
                        from_manual_serialized[i].data_cell.component_name
                    );
                    CHECK(from_builder_serialized[i].data_cell.buffer->Equals(
                        *from_manual_serialized[i].data_cell.buffer
                    ));
                }
            }
        }
    }
}
