// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include <arrow/api.h>

#include "affix_fuzzer17.hpp"

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer17::to_arrow_datatype() {
            return arrow::list(arrow::field(
                "item",
                arrow::dense_union({
                    arrow::field("_null_markers", arrow::null(), true, nullptr),
                    arrow::field("degrees", arrow::float32(), false, nullptr),
                    arrow::field("radians", arrow::float32(), false, nullptr),
                    arrow::field(
                        "craziness",
                        arrow::list(arrow::field(
                            "item",
                            arrow::struct_({
                                arrow::field(
                                    "single_float_optional", arrow::float32(), true, nullptr),
                                arrow::field(
                                    "single_string_required", arrow::utf8(), false, nullptr),
                                arrow::field(
                                    "single_string_optional", arrow::utf8(), true, nullptr),
                                arrow::field("many_floats_optional",
                                             arrow::list(arrow::field(
                                                 "item", arrow::float32(), true, nullptr)),
                                             true,
                                             nullptr),
                                arrow::field("many_strings_required",
                                             arrow::list(arrow::field(
                                                 "item", arrow::utf8(), false, nullptr)),
                                             false,
                                             nullptr),
                                arrow::field(
                                    "many_strings_optional",
                                    arrow::list(arrow::field("item", arrow::utf8(), true, nullptr)),
                                    true,
                                    nullptr),
                                arrow::field("flattened_scalar", arrow::float32(), false, nullptr),
                                arrow::field(
                                    "almost_flattened_scalar",
                                    arrow::struct_({
                                        arrow::field("value", arrow::float32(), false, nullptr),
                                    }),
                                    false,
                                    nullptr),
                                arrow::field("from_parent", arrow::boolean(), true, nullptr),
                            }),
                            false,
                            nullptr)),
                        false,
                        nullptr),
                    arrow::field("fixed_size_shenanigans",
                                 arrow::fixed_size_list(
                                     arrow::field("item", arrow::float32(), false, nullptr), 3),
                                 false,
                                 nullptr),
                }),
                true,
                nullptr));
        }
    } // namespace components
} // namespace rr
