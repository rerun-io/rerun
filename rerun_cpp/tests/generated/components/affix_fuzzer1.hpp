// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer1.hpp"

#include <cstdint>
#include <memory>
#include <rerun/result.hpp>
#include <utility>

namespace rerun::components {
    struct AffixFuzzer1 {
        rerun::datatypes::AffixFuzzer1 single_required;

      public:
        AffixFuzzer1() = default;

        AffixFuzzer1(rerun::datatypes::AffixFuzzer1 single_required_)
            : single_required(std::move(single_required_)) {}

        AffixFuzzer1& operator=(rerun::datatypes::AffixFuzzer1 single_required_) {
            single_required = std::move(single_required_);
            return *this;
        }

        /// Cast to the underlying AffixFuzzer1 datatype
        operator rerun::datatypes::AffixFuzzer1() const {
            return single_required;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::AffixFuzzer1) == sizeof(components::AffixFuzzer1));

    /// \private
    template <>
    struct Loggable<components::AffixFuzzer1> {
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer1";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::AffixFuzzer1>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::AffixFuzzer1` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer1* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::AffixFuzzer1>::to_arrow(
                &instances->single_required,
                num_instances
            );
        }
    };
} // namespace rerun
