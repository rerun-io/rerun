#pragma once

#include "error.hpp"

#include <string>

struct rr_timeline;

namespace rerun {
    /// Describes the type of a timeline or time point.
    enum class TimeType {
        /// Used e.g. for frames in a film.
        Sequence = 1,

        /// Nanoseconds.
        Duration = 2,

        /// Nanoseconds since Unix epoch (1970-01-01 00:00:00 UTC).
        Timestamp = 3,
    };

    /// Definition of a timeline.
    struct Timeline {
        /// The name of the timeline.
        std::string name;

        /// The type of the timeline.
        TimeType type;

        /// Creates a new timeline.
        Timeline(std::string name_, TimeType type_) : name(std::move(name_)), type(type_) {}

        Timeline() = delete;

        /// To rerun C API timeline.
        Error to_c_ffi_struct(rr_timeline& out_column) const;
    };
} // namespace rerun
