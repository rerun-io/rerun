#include "component_type.hpp"
#include "c/rerun.h"
#include "string_utils.hpp"

#include <arrow/c/bridge.h>

namespace rerun {
    Result<ComponentTypeHandle> ComponentType::register_component() const {
        rr_component_type type;
        type.name = detail::to_rr_string(name);
        ARROW_RETURN_NOT_OK(arrow::ExportType(*arrow_datatype, &type.schema));

        rr_error error = {};
        auto handle = rr_register_component_type(type, &error);
        if (error.code != RR_ERROR_CODE_OK) {
            return Error(error);
        }

        return handle;
    }
} // namespace rerun
