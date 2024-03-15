// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer1.hpp"
#include "affix_fuzzer4.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/result.hpp>
#include <utility>

namespace arrow {
    class StructBuilder;
}

namespace rerun::components {
    struct AffixFuzzer4 {
        std::optional<rerun::datatypes::AffixFuzzer1> single_optional;

      public:
        AffixFuzzer4() = default;

        AffixFuzzer4(std::optional<rerun::datatypes::AffixFuzzer1> single_optional_)
            : single_optional(std::move(single_optional_)) {}

        AffixFuzzer4& operator=(std::optional<rerun::datatypes::AffixFuzzer1> single_optional_) {
            single_optional = std::move(single_optional_);
            return *this;
        }

        /// Cast to the underlying AffixFuzzer1 datatype
        operator std::optional<rerun::datatypes::AffixFuzzer1>() const {
            return single_optional;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::AffixFuzzer1) == sizeof(rerun::components::AffixFuzzer4)
    );

    /// \private
    template <>
    struct Loggable<components::AffixFuzzer4> {
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer4";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::AffixFuzzer1>::arrow_datatype();
        }

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const components::AffixFuzzer4* elements,
            size_t num_elements
        ) {
            return Loggable<rerun::datatypes::AffixFuzzer1>::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::AffixFuzzer1*>(elements),
                num_elements
            );
        }

        /// Serializes an array of `rerun::components::AffixFuzzer4` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer4* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::AffixFuzzer1>::to_arrow(
                reinterpret_cast<const rerun::datatypes::AffixFuzzer1*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
