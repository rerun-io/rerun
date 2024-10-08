# Builds rerun_c from source.

# Determine Rust's librerun path.
if(APPLE)
    set(RERUN_C_BUILD_ARTIFACT ${PROJECT_SOURCE_DIR}/target/$ENV{CARGO_BUILD_TARGET}/release/librerun_c.a)
elseif(UNIX) # if(LINUX) # CMake 3.25
    set(RERUN_C_BUILD_ARTIFACT ${PROJECT_SOURCE_DIR}/target/$ENV{CARGO_BUILD_TARGET}/release/librerun_c.a)
elseif(WIN32)
    set(RERUN_C_BUILD_ARTIFACT ${PROJECT_SOURCE_DIR}/target/$ENV{CARGO_BUILD_TARGET}/release/rerun_c.lib)
else()
    message(FATAL_ERROR "Unsupported platform.")
endif()

# Setup rerun_c library
add_library(rerun_c STATIC IMPORTED GLOBAL)
set_target_properties(rerun_c PROPERTIES IMPORTED_LOCATION ${RERUN_C_BUILD_ARTIFACT})

# Just depend on all rust and toml files, it's hard to know which files exactly are relevant.
file(GLOB_RECURSE RERUN_C_SOURCES LIST_DIRECTORIES FALSE "${PROJECT_SOURCE_DIR}/crates/*.rs" "${PROJECT_SOURCE_DIR}/crates/*.toml")
add_custom_command(
    OUTPUT ${RERUN_C_BUILD_ARTIFACT}
    DEPENDS ${RERUN_C_SOURCES}
    COMMAND cargo build --release -p rerun_c
    COMMENT "Building rerun_c from source"
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
)

# In CMake you can't depend on an output file directly. We have to wrap this in a target that rerun_c then depends on.
add_custom_target(rerun_c_build DEPENDS "${RERUN_C_BUILD_ARTIFACT}")
add_dependencies(rerun_c rerun_c_build)
