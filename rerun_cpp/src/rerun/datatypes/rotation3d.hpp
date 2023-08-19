// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/rotation3d.fbs"

#pragma once

#include "../result.hpp"
#include "quaternion.hpp"
#include "rotation_axis_angle.hpp"

#include <cstdint>
#include <cstring>
#include <utility>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        namespace detail {
            enum class Rotation3DTag : uint8_t {
                /// Having a special empty state makes it possible to implement move-semantics. We
                /// need to be able to leave the object in a state which we can run the destructor
                /// on.
                NONE = 0,
                Quaternion,
                AxisAngle,
            };

            union Rotation3DData {
                /// Rotation defined by a quaternion.
                rerun::datatypes::Quaternion quaternion;

                /// Rotation defined with an axis and an angle.
                rerun::datatypes::RotationAxisAngle axis_angle;

                Rotation3DData() {}

                ~Rotation3DData() {}

                void swap(Rotation3DData &other) noexcept {
                    // This bitwise swap would fail for self-referential types, but we don't have
                    // any of those.
                    char temp[sizeof(Rotation3DData)];
                    void *otherbytes = reinterpret_cast<void *>(&other);
                    void *thisbytes = reinterpret_cast<void *>(this);
                    std::memcpy(temp, thisbytes, sizeof(Rotation3DData));
                    std::memcpy(thisbytes, otherbytes, sizeof(Rotation3DData));
                    std::memcpy(otherbytes, temp, sizeof(Rotation3DData));
                }
            };
        } // namespace detail

        /// A 3D rotation.
        struct Rotation3D {
            Rotation3D() : _tag(detail::Rotation3DTag::NONE) {}

            Rotation3D(const Rotation3D &other) : _tag(other._tag) {
                const void *otherbytes = reinterpret_cast<const void *>(&other._data);
                void *thisbytes = reinterpret_cast<void *>(&this->_data);
                std::memcpy(thisbytes, otherbytes, sizeof(detail::Rotation3DData));
            }

            Rotation3D &operator=(const Rotation3D &other) noexcept {
                Rotation3D tmp(other);
                this->swap(tmp);
                return *this;
            }

            Rotation3D(Rotation3D &&other) noexcept : _tag(detail::Rotation3DTag::NONE) {
                this->swap(other);
            }

            Rotation3D &operator=(Rotation3D &&other) noexcept {
                this->swap(other);
                return *this;
            }

            void swap(Rotation3D &other) noexcept {
                auto tag_temp = this->_tag;
                this->_tag = other._tag;
                other._tag = tag_temp;
                this->_data.swap(other._data);
            }

            /// Rotation defined by a quaternion.
            static Rotation3D quaternion(rerun::datatypes::Quaternion quaternion) {
                Rotation3D self;
                self._tag = detail::Rotation3DTag::Quaternion;
                self._data.quaternion = std::move(quaternion);
                return self;
            }

            /// Rotation defined with an axis and an angle.
            static Rotation3D axis_angle(rerun::datatypes::RotationAxisAngle axis_angle) {
                Rotation3D self;
                self._tag = detail::Rotation3DTag::AxisAngle;
                self._data.axis_angle = std::move(axis_angle);
                return self;
            }

            /// Rotation defined by a quaternion.
            Rotation3D(rerun::datatypes::Quaternion quaternion) {
                *this = Rotation3D::quaternion(std::move(quaternion));
            }

            /// Rotation defined with an axis and an angle.
            Rotation3D(rerun::datatypes::RotationAxisAngle axis_angle) {
                *this = Rotation3D::axis_angle(std::move(axis_angle));
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType> &to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool *memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DenseUnionBuilder *builder, const Rotation3D *elements, size_t num_elements
            );

          private:
            detail::Rotation3DTag _tag;
            detail::Rotation3DData _data;
        };
    } // namespace datatypes
} // namespace rerun
