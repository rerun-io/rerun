// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/quaternion.fbs"

#pragma once

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
}

namespace rr {
    namespace datatypes {
        /// A Quaternion represented by 4 real numbers.
        struct Quaternion {
            float xyzw[4];

          public:
            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace datatypes
} // namespace rr
