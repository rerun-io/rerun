#pragma once

#include "error.hpp"

#include <string>

struct rr_timeline;

namespace rerun {
    /// Describes the type of a timeline or time point.
    enum class TimeType {
        Time = 0,
        Sequence = 1,
    };

    /// Definition of a timeline.
    struct Timeline {
        /// The name of the timeline.
        std::string name;

        /// The type of the timeline.
        TimeType type;

      public:
        /// Creates a new timeline.
        Timeline(std::string _name, TimeType _type = TimeType::Time)
            : name(std::move(_name)), type(_type) {}

        Timeline() = delete;

        /// To rerun C API timeline.
        Error to_c_ffi_struct(rr_timeline& out_column) const;
    };
} // namespace rerun
