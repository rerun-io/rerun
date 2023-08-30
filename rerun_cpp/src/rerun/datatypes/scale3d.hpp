// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/scale3d.fbs"

#pragma once

#include "../result.hpp"
#include "vec3d.hpp"

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
            enum class Scale3DTag : uint8_t {
                /// Having a special empty state makes it possible to implement move-semantics. We
                /// need to be able to leave the object in a state which we can run the destructor
                /// on.
                NONE = 0,
                ThreeD,
                Uniform,
            };

            union Scale3DData {
                /// Individual scaling factors for each axis, distorting the original object.
                rerun::datatypes::Vec3D three_d;

                /// Uniform scaling factor along all axis.
                float uniform;

                Scale3DData() {}

                ~Scale3DData() {}

                void swap(Scale3DData &other) noexcept {
                    // This bitwise swap would fail for self-referential types, but we don't have
                    // any of those.
                    char temp[sizeof(Scale3DData)];
                    void *otherbytes = reinterpret_cast<void *>(&other);
                    void *thisbytes = reinterpret_cast<void *>(this);
                    std::memcpy(temp, thisbytes, sizeof(Scale3DData));
                    std::memcpy(thisbytes, otherbytes, sizeof(Scale3DData));
                    std::memcpy(otherbytes, temp, sizeof(Scale3DData));
                }
            };
        } // namespace detail

        /// 3D scaling factor, part of a transform representation.
        struct Scale3D {
            Scale3D() : _tag(detail::Scale3DTag::NONE) {}

            Scale3D(const Scale3D &other) : _tag(other._tag) {
                const void *otherbytes = reinterpret_cast<const void *>(&other._data);
                void *thisbytes = reinterpret_cast<void *>(&this->_data);
                std::memcpy(thisbytes, otherbytes, sizeof(detail::Scale3DData));
            }

            Scale3D &operator=(const Scale3D &other) noexcept {
                Scale3D tmp(other);
                this->swap(tmp);
                return *this;
            }

            Scale3D(Scale3D &&other) noexcept : _tag(detail::Scale3DTag::NONE) {
                this->swap(other);
            }

            Scale3D &operator=(Scale3D &&other) noexcept {
                this->swap(other);
                return *this;
            }

            void swap(Scale3D &other) noexcept {
                auto tag_temp = this->_tag;
                this->_tag = other._tag;
                other._tag = tag_temp;
                this->_data.swap(other._data);
            }

            /// Individual scaling factors for each axis, distorting the original object.
            static Scale3D three_d(rerun::datatypes::Vec3D three_d) {
                Scale3D self;
                self._tag = detail::Scale3DTag::ThreeD;
                self._data.three_d = std::move(three_d);
                return self;
            }

            /// Uniform scaling factor along all axis.
            static Scale3D uniform(float uniform) {
                Scale3D self;
                self._tag = detail::Scale3DTag::Uniform;
                self._data.uniform = std::move(uniform);
                return self;
            }

            /// Individual scaling factors for each axis, distorting the original object.
            Scale3D(rerun::datatypes::Vec3D three_d) {
                *this = Scale3D::three_d(std::move(three_d));
            }

            /// Uniform scaling factor along all axis.
            Scale3D(float uniform) {
                *this = Scale3D::uniform(std::move(uniform));
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType> &arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool *memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DenseUnionBuilder *builder, const Scale3D *elements, size_t num_elements
            );

          private:
            detail::Scale3DTag _tag;
            detail::Scale3DData _data;
        };
    } // namespace datatypes
} // namespace rerun
