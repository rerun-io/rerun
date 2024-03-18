// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer1.hpp"

#include <cstdint>
#include <memory>
#include <rerun/result.hpp>
#include <utility>

namespace rerun::components {
    struct AffixFuzzer3 {
        rerun::datatypes::AffixFuzzer1 single_required;

      public:
        AffixFuzzer3() = default;

        AffixFuzzer3(rerun::datatypes::AffixFuzzer1 single_required_)
            : single_required(std::move(single_required_)) {}

        AffixFuzzer3& operator=(rerun::datatypes::AffixFuzzer1 single_required_) {
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
    /// \private
    template <>
    struct Loggable<components::AffixFuzzer3> {
        using TypeFwd = rerun::datatypes::AffixFuzzer1;
        static_assert(sizeof(TypeFwd) == sizeof(components::AffixFuzzer3));
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer3";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<TypeFwd>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::AffixFuzzer3` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer3* instances, size_t num_instances
        ) {
            return Loggable<TypeFwd>::to_arrow(
                reinterpret_cast<const TypeFwd*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
