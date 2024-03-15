// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer4.hpp"
#include "../datatypes/affix_fuzzer5.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/result.hpp>
#include <utility>

namespace arrow {
    class StructBuilder;
}

namespace rerun::components {
    struct AffixFuzzer19 {
        rerun::datatypes::AffixFuzzer5 just_a_table_nothing_shady;

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
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::AffixFuzzer5) == sizeof(components::AffixFuzzer19));

    /// \private
    template <>
    struct Loggable<components::AffixFuzzer19> {
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer19";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::AffixFuzzer5>::arrow_datatype();
        }

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const components::AffixFuzzer19* elements,
            size_t num_elements
        ) {
            return Loggable<rerun::datatypes::AffixFuzzer5>::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::AffixFuzzer5*>(elements),
                num_elements
            );
        }

        /// Serializes an array of `rerun::components::AffixFuzzer19` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer19* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::AffixFuzzer5>::to_arrow(
                reinterpret_cast<const rerun::datatypes::AffixFuzzer5*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
