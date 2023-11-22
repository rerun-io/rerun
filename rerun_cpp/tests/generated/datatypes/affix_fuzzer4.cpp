// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#include "affix_fuzzer4.hpp"

#include "affix_fuzzer3.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::datatypes {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<datatypes::AffixFuzzer4>::arrow_datatype() {
        static const auto datatype = arrow::dense_union({
            arrow::field("_null_markers", arrow::null(), true, nullptr),
            arrow::field(
                "single_required",
                Loggable<rerun::datatypes::AffixFuzzer3>::arrow_datatype(),
                false
            ),
            arrow::field(
                "many_required",
                arrow::list(arrow::field(
                    "item",
                    Loggable<rerun::datatypes::AffixFuzzer3>::arrow_datatype(),
                    false
                )),
                false
            ),
            arrow::field(
                "many_optional",
                arrow::list(arrow::field(
                    "item",
                    Loggable<rerun::datatypes::AffixFuzzer3>::arrow_datatype(),
                    false
                )),
                true
            ),
        });
        return datatype;
    }

    rerun::Error Loggable<datatypes::AffixFuzzer4>::fill_arrow_array_builder(
        arrow::DenseUnionBuilder* builder, const datatypes::AffixFuzzer4* elements,
        size_t num_elements
    ) {
        if (builder == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
        }
        if (elements == nullptr) {
            return rerun::Error(
                ErrorCode::UnexpectedNullArgument,
                "Cannot serialize null pointer to arrow array."
            );
        }

        ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
        for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
            const auto& union_instance = elements[elem_idx];
            ARROW_RETURN_NOT_OK(builder->Append(static_cast<int8_t>(union_instance.get_union_tag()))
            );

            auto variant_index = static_cast<int>(union_instance.get_union_tag());
            auto variant_builder_untyped = builder->child_builder(variant_index).get();

            using TagType = datatypes::detail::AffixFuzzer4Tag;
            switch (union_instance.get_union_tag()) {
                case TagType::None: {
                    ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                } break;
                case TagType::single_required: {
                    auto variant_builder =
                        static_cast<arrow::DenseUnionBuilder*>(variant_builder_untyped);
                    RR_RETURN_NOT_OK(
                        Loggable<rerun::datatypes::AffixFuzzer3>::fill_arrow_array_builder(
                            variant_builder,
                            &union_instance.get_union_data().single_required,
                            1
                        )
                    );
                } break;
                case TagType::many_required: {
                    auto variant_builder =
                        static_cast<arrow::ListBuilder*>(variant_builder_untyped);
                    (void)variant_builder;
                    return rerun::Error(
                        ErrorCode::NotImplemented,
                        "Failed to serialize AffixFuzzer4::many_required: objects (Object(\"rerun.testing.datatypes.AffixFuzzer3\")) in unions not yet implemented"
                    );
                } break;
                case TagType::many_optional: {
                    auto variant_builder =
                        static_cast<arrow::ListBuilder*>(variant_builder_untyped);
                    (void)variant_builder;
                    return rerun::Error(
                        ErrorCode::NotImplemented,
                        "Failed to serialize AffixFuzzer4::many_optional: nullable list types in unions not yet implemented"
                    );
                } break;
            }
        }

        return Error::ok();
    }

    Result<rerun::DataCell> Loggable<datatypes::AffixFuzzer4>::to_arrow(
        const datatypes::AffixFuzzer4* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<datatypes::AffixFuzzer4>::fill_arrow_array_builder(
                static_cast<arrow::DenseUnionBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        static const Result<ComponentTypeHandle> component_type =
            ComponentType(Name, datatype).register_component();
        RR_RETURN_NOT_OK(component_type.error);

        DataCell cell;
        cell.num_instances = num_instances;
        cell.array = std::move(array);
        cell.component_type = component_type.value;
        return cell;
    }
} // namespace rerun
