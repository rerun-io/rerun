// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer4.hpp"
#include "../datatypes/affix_fuzzer5.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <utility>

namespace arrow {
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::components {
    struct AffixFuzzer19 {
        rerun::datatypes::AffixFuzzer5 just_a_table_nothing_shady;

        /// Name of the component, used for serialization.
        static const char NAME[];

      public:
        AffixFuzzer19() = default;

        AffixFuzzer19(rerun::datatypes::AffixFuzzer5 just_a_table_nothing_shady_)
            : just_a_table_nothing_shady(std::move(just_a_table_nothing_shady_)) {}

        AffixFuzzer19& operator=(rerun::datatypes::AffixFuzzer5 just_a_table_nothing_shady_) {
            just_a_table_nothing_shady = std::move(just_a_table_nothing_shady_);
            return *this;
        }

        AffixFuzzer19(std::optional<rerun::datatypes::AffixFuzzer4> single_optional_union_)
            : just_a_table_nothing_shady(std::move(single_optional_union_)) {}

        AffixFuzzer19& operator=(
            std::optional<rerun::datatypes::AffixFuzzer4> single_optional_union_
        ) {
            just_a_table_nothing_shady = std::move(single_optional_union_);
            return *this;
        }

        /// Cast to the underlying AffixFuzzer5 datatype
        operator rerun::datatypes::AffixFuzzer5() const {
            return just_a_table_nothing_shady;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const AffixFuzzer19* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of AffixFuzzer19 components.
        static Result<rerun::DataCell> to_data_cell(
            const AffixFuzzer19* instances, size_t num_instances
        );
    };
} // namespace rerun::components
