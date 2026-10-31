#include <catch2/catch_test_macros.hpp>

#include <arrow/api.h>

#include <rerun/archetype_utils.hpp>

using namespace rerun;

namespace {
    ComponentBatch int_batch(const std::vector<int32_t>& values, ComponentTypeHandle handle) {
        arrow::Int32Builder builder;
        REQUIRE(builder.AppendValues(values).ok());
        return ComponentBatch{builder.Finish().ValueOrDie(), handle};
    }
} // namespace

SCENARIO("Collecting batches preserves set fields in order", "[archetype_utils]") {
    GIVEN("an absent field between a nonempty batch and a present empty batch") {
        std::optional<ComponentBatch> first = int_batch({10, 20}, 11);
        const std::optional<ComponentBatch> absent;
        std::optional<ComponentBatch> empty = int_batch({}, 22);

        WHEN("the fields are collected") {
            auto batches = detail::collect_batches({&first, &absent, &empty}).value_or_throw();

            THEN("only the set fields are returned, sharing their arrays and handles") {
                REQUIRE(batches.size() == 2);
                CHECK(batches[0].array == first->array);
                CHECK(batches[0].component_type == 11);
                CHECK(batches[1].array == empty->array);
                CHECK(batches[1].component_type == 22);
            }
        }
    }
}

SCENARIO("Explicit partitions retain empty rows and source values", "[archetype_utils]") {
    GIVEN("a batch with two values") {
        std::optional<ComponentBatch> batch = int_batch({10, 20}, 11);

        WHEN("the batch is partitioned with an empty middle row") {
            auto columns = detail::partition_batches({&batch}, {1, 0, 1}).value_or_throw();

            THEN("the column has the requested rows and shares its source values") {
                REQUIRE(columns.size() == 1);
                auto list = std::static_pointer_cast<arrow::ListArray>(columns[0].array);
                REQUIRE(list->ValidateFull().ok());
                REQUIRE(list->length() == 3);
                CHECK(list->value_length(0) == 1);
                CHECK(list->value_length(1) == 0);
                CHECK(list->value_length(2) == 1);
                CHECK(list->values()->data() == batch->array->data());
                CHECK(columns[0].component_type == batch->component_type);
            }
        }
    }
}

SCENARIO("Partitioned columns share offsets and retain their values", "[archetype_utils]") {
    GIVEN("two nonempty batches with an absent field between them") {
        std::optional<ComponentBatch> first = int_batch({10, 20}, 11);
        const std::optional<ComponentBatch> absent;
        std::optional<ComponentBatch> second = int_batch({30, 40}, 22);
        std::weak_ptr<arrow::ArrayData> first_values = first->array->data();
        std::weak_ptr<arrow::ArrayData> second_values = second->array->data();

        auto check_columns = [&](const Collection<ComponentColumn>& columns) {
            REQUIRE(columns.size() == 2);
            auto first_list = std::static_pointer_cast<arrow::ListArray>(columns[0].array);
            auto second_list = std::static_pointer_cast<arrow::ListArray>(columns[1].array);
            CHECK(first_list->value_offsets() == second_list->value_offsets());
            REQUIRE_FALSE(first_values.expired());
            REQUIRE_FALSE(second_values.expired());
            CHECK(first_list->values()->data() == first_values.lock());
            CHECK(second_list->values()->data() == second_values.lock());
            CHECK(columns[0].component_type == 11);
            CHECK(columns[1].component_type == 22);
            CHECK(first_list->ValidateFull().ok());
            CHECK(second_list->ValidateFull().ok());
        };

        WHEN("explicit lengths partition the fields and the source batches are destroyed") {
            auto columns =
                detail::partition_batches({&first, &absent, &second}, {1, 0, 1}).value_or_throw();
            first.reset();
            second.reset();

            THEN("the columns share offsets and retain their original values and handles") {
                check_columns(columns);
            }
        }

        WHEN("unit lengths partition the fields and the source batches are destroyed") {
            auto columns = detail::partition_batches_of_unit_length({&first, &absent, &second})
                               .value_or_throw();
            first.reset();
            second.reset();

            THEN("the columns share offsets and retain their original values and handles") {
                check_columns(columns);
            }
        }
    }
}

SCENARIO("Unit partitioning uses the first set field's length", "[archetype_utils]") {
    GIVEN("an absent field, an empty batch, and a nonempty batch") {
        const std::optional<ComponentBatch> absent;
        std::optional<ComponentBatch> empty = int_batch({}, 11);
        std::optional<ComponentBatch> values = int_batch({10, 20}, 22);

        WHEN("only the nonempty batch is set") {
            auto columns =
                detail::partition_batches_of_unit_length({&absent, &values}).value_or_throw();

            THEN("each value gets its own row") {
                REQUIRE(columns.size() == 1);
                auto list = std::static_pointer_cast<arrow::ListArray>(columns[0].array);
                REQUIRE(list->length() == 2);
                CHECK(list->value_length(0) == 1);
                CHECK(list->value_length(1) == 1);
            }
        }

        WHEN("the empty batch is the first set field") {
            auto columns = detail::partition_batches_of_unit_length({&absent, &empty, &values})
                               .value_or_throw();

            THEN("both columns have no rows") {
                REQUIRE(columns.size() == 2);
                CHECK(columns[0].array->length() == 0);
                CHECK(columns[1].array->length() == 0);
            }
        }
    }
}
