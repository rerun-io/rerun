// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/transform3d.fbs".

#pragma once

#include "../result.hpp"
#include "translation_and_mat3x3.hpp"
#include "translation_rotation_scale3d.hpp"

#include <cstdint>
#include <cstring>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        namespace detail {
            enum class Transform3DTag : uint8_t {
                /// Having a special empty state makes it possible to implement move-semantics. We need to be able to leave the object in a state which we can run the destructor on.
                NONE = 0,
                TranslationAndMat3x3,
                TranslationRotationScale,
            };

            union Transform3DData {
                rerun::datatypes::TranslationAndMat3x3 translation_and_mat3x3;

                rerun::datatypes::TranslationRotationScale3D translation_rotation_scale;

                Transform3DData() {}

                ~Transform3DData() {}

                void swap(Transform3DData& other) noexcept {
                    // This bitwise swap would fail for self-referential types, but we don't have any of those.
                    char temp[sizeof(Transform3DData)];
                    void* otherbytes = reinterpret_cast<void*>(&other);
                    void* thisbytes = reinterpret_cast<void*>(this);
                    std::memcpy(temp, thisbytes, sizeof(Transform3DData));
                    std::memcpy(thisbytes, otherbytes, sizeof(Transform3DData));
                    std::memcpy(otherbytes, temp, sizeof(Transform3DData));
                }
            };
        } // namespace detail

        /// **Datatype**: Representation of a 3D affine transform.
        struct Transform3D {
            Transform3D() : _tag(detail::Transform3DTag::NONE) {}

            Transform3D(const Transform3D& other) : _tag(other._tag) {
                const void* otherbytes = reinterpret_cast<const void*>(&other._data);
                void* thisbytes = reinterpret_cast<void*>(&this->_data);
                std::memcpy(thisbytes, otherbytes, sizeof(detail::Transform3DData));
            }

            Transform3D& operator=(const Transform3D& other) noexcept {
                Transform3D tmp(other);
                this->swap(tmp);
                return *this;
            }

            Transform3D(Transform3D&& other) noexcept : Transform3D() {
                this->swap(other);
            }

            Transform3D& operator=(Transform3D&& other) noexcept {
                this->swap(other);
                return *this;
            }

            void swap(Transform3D& other) noexcept {
                std::swap(this->_tag, other._tag);
                this->_data.swap(other._data);
            }

            static Transform3D translation_and_mat3x3(
                rerun::datatypes::TranslationAndMat3x3 translation_and_mat3x3
            ) {
                Transform3D self;
                self._tag = detail::Transform3DTag::TranslationAndMat3x3;
                self._data.translation_and_mat3x3 = std::move(translation_and_mat3x3);
                return self;
            }

            static Transform3D translation_rotation_scale(
                rerun::datatypes::TranslationRotationScale3D translation_rotation_scale
            ) {
                Transform3D self;
                self._tag = detail::Transform3DTag::TranslationRotationScale;
                self._data.translation_rotation_scale = std::move(translation_rotation_scale);
                return self;
            }

            Transform3D(rerun::datatypes::TranslationAndMat3x3 translation_and_mat3x3) {
                *this = Transform3D::translation_and_mat3x3(std::move(translation_and_mat3x3));
            }

            Transform3D(rerun::datatypes::TranslationRotationScale3D translation_rotation_scale) {
                *this =
                    Transform3D::translation_rotation_scale(std::move(translation_rotation_scale));
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DenseUnionBuilder* builder, const Transform3D* elements, size_t num_elements
            );

          private:
            detail::Transform3DTag _tag;
            detail::Transform3DData _data;
        };
    } // namespace datatypes
} // namespace rerun
