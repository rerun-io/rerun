// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/point2d.fbs"

#pragma once

#include <cstdint>
#include <memory>
#include <utility>

#include "../datatypes/point2d.hpp"

namespace arrow {
    class DataType;
}

namespace rr {
    namespace components {
        /// A point in 2D space.
        struct Point2D {
            rr::datatypes::Point2D xy;

          public:
            Point2D(rr::datatypes::Point2D xy) : xy(std::move(xy)) {}

            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace components
} // namespace rr
