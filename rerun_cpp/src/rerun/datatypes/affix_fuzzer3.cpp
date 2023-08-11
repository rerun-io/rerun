// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#include "affix_fuzzer3.hpp"

#include "../datatypes/affix_fuzzer1.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &AffixFuzzer3::to_arrow_datatype() {
            static const auto datatype = arrow::dense_union({
                arrow::field("_null_markers", arrow::null(), true, nullptr),
                arrow::field("degrees", arrow::float32(), false),
                arrow::field("radians", arrow::float32(), true),
                arrow::field(
                    "craziness",
                    arrow::list(arrow::field(
                        "item",
                        rerun::datatypes::AffixFuzzer1::to_arrow_datatype(),
                        false
                    )),
                    false
                ),
                arrow::field(
                    "fixed_size_shenanigans",
                    arrow::fixed_size_list(arrow::field("item", arrow::float32(), false), 3),
                    false
                ),
            });
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::DenseUnionBuilder>>
            AffixFuzzer3::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::DenseUnionBuilder>(
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::NullBuilder>(memory_pool),
                    std::make_shared<arrow::FloatBuilder>(memory_pool),
                    std::make_shared<arrow::FloatBuilder>(memory_pool),
                    std::make_shared<arrow::ListBuilder>(
                        memory_pool,
                        rerun::datatypes::AffixFuzzer1::new_arrow_array_builder(memory_pool)
                            .ValueOrDie()
                    ),
                    std::make_shared<arrow::FixedSizeListBuilder>(
                        memory_pool,
                        std::make_shared<arrow::FloatBuilder>(memory_pool),
                        3
                    ),
                }),
                to_arrow_datatype()
            ));
        }

        arrow::Status AffixFuzzer3::fill_arrow_array_builder(
            arrow::DenseUnionBuilder *builder, const AffixFuzzer3 *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            ARROW_RETURN_NOT_OK(builder->Reserve(num_elements));
            for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &union_instance = elements[elem_idx];
                ARROW_RETURN_NOT_OK(builder->Append(static_cast<uint8_t>(union_instance._tag)));

                auto variant_index = static_cast<int>(union_instance._tag);
                auto variant_builder_untyped = builder->child_builder(variant_index).get();

                switch (union_instance._tag) {
                    case detail::AffixFuzzer3Tag::NONE: {
                        ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                        break;
                    }
                    case detail::AffixFuzzer3Tag::degrees: {
                        auto variant_builder =
                            static_cast<arrow::FloatBuilder *>(variant_builder_untyped);
                        ARROW_RETURN_NOT_OK(variant_builder->Append(union_instance._data.degrees));
                        break;
                    }
                    case detail::AffixFuzzer3Tag::radians: {
                        auto variant_builder =
                            static_cast<arrow::FloatBuilder *>(variant_builder_untyped);
                        const auto &element = union_instance._data;
                        if (element.radians.has_value()) {
                            ARROW_RETURN_NOT_OK(variant_builder->Append(element.radians.value()));
                        } else {
                            ARROW_RETURN_NOT_OK(variant_builder->AppendNull());
                        }
                        break;
                    }
                    case detail::AffixFuzzer3Tag::craziness: {
                        auto variant_builder =
                            static_cast<arrow::ListBuilder *>(variant_builder_untyped);
                        return arrow::Status::NotImplemented(
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                    case detail::AffixFuzzer3Tag::fixed_size_shenanigans: {
                        auto variant_builder =
                            static_cast<arrow::FixedSizeListBuilder *>(variant_builder_untyped);
                        return arrow::Status::NotImplemented(
                            "TODO(andreas): list types in unions are not yet supported"
                        );
                        break;
                    }
                }
            }

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
