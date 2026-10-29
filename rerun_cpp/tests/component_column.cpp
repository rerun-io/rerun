#include <catch2/catch_test_macros.hpp>

#include <arrow/api.h>

#include <rerun/component_column.hpp>

using namespace rerun;

SCENARIO("Columns accept shared offset buffers", "[component_column]") {
    GIVEN("a batch and offsets for an empty middle row") {
        arrow::Int32Builder builder;
        REQUIRE(builder.AppendValues({10, 20}).ok());
        ComponentBatch batch{builder.Finish().ValueOrDie(), 11};
        auto offsets = detail::offsets_from_lengths({1, 0, 1});

        WHEN("a column is created from the shared buffer") {
            auto column =
                ComponentColumn::from_batch_with_offsets(batch, offsets, 3).value_or_throw();

            THEN("it retains the buffer and matches the collection-based constructors") {
                auto list = std::static_pointer_cast<arrow::ListArray>(column.array);
                CHECK(list->value_offsets() == offsets);
                CHECK(list->values()->data() == batch.array->data());
                CHECK(column.component_type == batch.component_type);
                CHECK(list->ValidateFull().ok());

                auto from_lengths =
                    ComponentColumn::from_batch_with_lengths(batch, {1, 0, 1}).value_or_throw();
                auto from_offsets =
                    ComponentColumn::from_batch_with_offsets(batch, {0, 1, 1, 2}).value_or_throw();
                CHECK(column.array->Equals(from_lengths.array));
                CHECK(column.array->Equals(from_offsets.array));
            }
        }
    }
}
