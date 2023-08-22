// The Rerun C++ SDK.
#pragma once

// Built-in Rerun types (largely generated from an interface definition language)
#include "rerun/archetypes.hpp"
#include "rerun/components.hpp"
#include "rerun/datatypes.hpp"

// Rerun API.
#include "rerun/error.hpp"
#include "rerun/recording_stream.hpp"
#include "rerun/result.hpp"
#include "rerun/sdk_info.hpp"

// Archetypes are the quick-and-easy default way of logging data to Rerun.
// Make them available in the rerun namespace.
namespace rerun {
    using namespace archetypes;
}
