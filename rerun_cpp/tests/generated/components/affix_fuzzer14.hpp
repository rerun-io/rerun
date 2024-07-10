// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer3.hpp"

#include <cstdint>
#include <memory>
#include <rerun/result.hpp>
#include <utility>

namespace rerun::components {
    struct AffixFuzzer14 {
        rerun::datatypes::AffixFuzzer3 single_required_union;

      public:
        AffixFuzzer14() = default;

        AffixFuzzer14(rerun::datatypes::AffixFuzzer3 single_required_union_)
            : single_required_union(std::move(single_required_union_)) {}

        AffixFuzzer14& operator=(rerun::datatypes::AffixFuzzer3 single_required_union_) {
            single_required_union = std::move(single_required_union_);
            return *this;
        }

        /// Cast to the underlying AffixFuzzer3 datatype
        operator rerun::datatypes::AffixFuzzer3() const {
            return single_required_union;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::AffixFuzzer3) == sizeof(components::AffixFuzzer14));

    /// \private
    template <>
    struct Loggable<components::AffixFuzzer14> {
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer14";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::AffixFuzzer3>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::AffixFuzzer14` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer14* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::AffixFuzzer3>::to_arrow(
                &instances->single_required_union,
                num_instances
            );
        }
    };
} // namespace rerun
