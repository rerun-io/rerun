#include "timeline.hpp"

#include "c/rerun.h"
#include "string_utils.hpp"

namespace rerun {
    Error Timeline::to_c_ffi_struct(rr_timeline& out_column) const {
        switch (type) {
            case TimeType::Time:
                out_column.type = RR_TIME_TYPE_TIME;
                break;
            case TimeType::Sequence:
                out_column.type = RR_TIME_TYPE_SEQUENCE;
                break;
            default:
                return Error(
                    ErrorCode::InvalidEnumValue,
                    "Invalid TimeType" + std::to_string(static_cast<int>(type))
                );
        }
        out_column.name = detail::to_rr_string(name);

        return Error::ok();
    }
} // namespace rerun
