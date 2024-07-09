// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/scale3d.fbs".

#pragma once

#include "../result.hpp"
#include "vec3d.hpp"

#include <cstdint>
#include <cstring>
#include <memory>
#include <new>
#include <utility>

namespace arrow {
    class Array;
    class DataType;
    class DenseUnionBuilder;
} // namespace arrow

namespace rerun::datatypes {
    namespace detail {
        /// \private
        enum class Scale3DTag : uint8_t {
            /// Having a special empty state makes it possible to implement move-semantics. We need to be able to leave the object in a state which we can run the destructor on.
            None = 0,
            ThreeD,
            Uniform,
        };

        /// \private
        union Scale3DData {
            /// Individual scaling factors for each axis, distorting the original object.
            rerun::datatypes::Vec3D three_d;

            /// Uniform scaling factor along all axis.
            float uniform;

            Scale3DData() {
                std::memset(reinterpret_cast<void*>(this), 0, sizeof(Scale3DData));
            }

            ~Scale3DData() {}

            void swap(Scale3DData& other) noexcept {
                // This bitwise swap would fail for self-referential types, but we don't have any of those.
                char temp[sizeof(Scale3DData)];
                void* otherbytes = reinterpret_cast<void*>(&other);
                void* thisbytes = reinterpret_cast<void*>(this);
                std::memcpy(temp, thisbytes, sizeof(Scale3DData));
                std::memcpy(thisbytes, otherbytes, sizeof(Scale3DData));
                std::memcpy(otherbytes, temp, sizeof(Scale3DData));
            }
        };
    } // namespace detail

    /// **Datatype**: 3D scaling factor, part of a transform representation.
    struct Scale3D {
        Scale3D() : _tag(detail::Scale3DTag::None) {}

        /// Copy constructor
        Scale3D(const Scale3D& other) : _tag(other._tag) {
            const void* otherbytes = reinterpret_cast<const void*>(&other._data);
            void* thisbytes = reinterpret_cast<void*>(&this->_data);
            std::memcpy(thisbytes, otherbytes, sizeof(detail::Scale3DData));
        }

        Scale3D& operator=(const Scale3D& other) noexcept {
            Scale3D tmp(other);
            this->swap(tmp);
            return *this;
        }

        Scale3D(Scale3D&& other) noexcept : Scale3D() {
            this->swap(other);
        }

        Scale3D& operator=(Scale3D&& other) noexcept {
            this->swap(other);
            return *this;
        }

        void swap(Scale3D& other) noexcept {
            std::swap(this->_tag, other._tag);
            this->_data.swap(other._data);
        }

        /// Individual scaling factors for each axis, distorting the original object.
        Scale3D(rerun::datatypes::Vec3D three_d) : Scale3D() {
            *this = Scale3D::three_d(std::move(three_d));
        }

        /// Uniform scaling factor along all axis.
        Scale3D(float uniform) : Scale3D() {
            *this = Scale3D::uniform(std::move(uniform));
        }

        /// Individual scaling factors for each axis, distorting the original object.
        static Scale3D three_d(rerun::datatypes::Vec3D three_d) {
            Scale3D self;
            self._tag = detail::Scale3DTag::ThreeD;
            new (&self._data.three_d) rerun::datatypes::Vec3D(std::move(three_d));
            return self;
        }

        /// Uniform scaling factor along all axis.
        static Scale3D uniform(float uniform) {
            Scale3D self;
            self._tag = detail::Scale3DTag::Uniform;
            new (&self._data.uniform) float(std::move(uniform));
            return self;
        }

        /// Return a pointer to three_d if the union is in that state, otherwise `nullptr`.
        const rerun::datatypes::Vec3D* get_three_d() const {
            if (_tag == detail::Scale3DTag::ThreeD) {
                return &_data.three_d;
            } else {
                return nullptr;
            }
        }

        /// Return a pointer to uniform if the union is in that state, otherwise `nullptr`.
        const float* get_uniform() const {
            if (_tag == detail::Scale3DTag::Uniform) {
                return &_data.uniform;
            } else {
                return nullptr;
            }
        }

        /// \private
        const detail::Scale3DData& get_union_data() const {
            return _data;
        }

        /// \private
        detail::Scale3DTag get_union_tag() const {
            return _tag;
        }

      private:
        detail::Scale3DTag _tag;
        detail::Scale3DData _data;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::Scale3D> {
        static constexpr const char Name[] = "rerun.datatypes.Scale3D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::Scale3D` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::Scale3D* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::DenseUnionBuilder* builder, const datatypes::Scale3D* elements,
            size_t num_elements
        );
    };
} // namespace rerun
