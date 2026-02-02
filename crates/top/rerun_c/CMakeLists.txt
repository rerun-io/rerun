# Builds rerun_c from source.

# Determine build configuration (Debug or Release)
string(TOLOWER "${CMAKE_BUILD_TYPE}" CMAKE_BUILD_TYPE_LOWER)

if(CMAKE_BUILD_TYPE_LOWER STREQUAL "debug")
    set(CARGO_BUILD_FLAG "")
    set(RUST_BUILD_TYPE "debug")
    message("Building DEBUG BUILD of Rerun C++ SDK")
else()
    message("Building RELEASE BUILD of Rerun C++ SDK")
    set(CARGO_BUILD_FLAG "--release")
    set(RUST_BUILD_TYPE "release")
endif()

# Determine Rust's librerun path.
if(DEFINED ENV{CARGO_TARGET_DIR})
    set(CARGO_TARGET_DIR $ENV{CARGO_TARGET_DIR})
else()
    set(CARGO_TARGET_DIR "target")
endif()

if(NOT IS_ABSOLUTE ${CARGO_TARGET_DIR})
    set(CARGO_TARGET_DIR ${PROJECT_SOURCE_DIR}/${CARGO_TARGET_DIR})
endif()

if(APPLE)
    set(RERUN_C_BUILD_ARTIFACT ${CARGO_TARGET_DIR}/$ENV{CARGO_BUILD_TARGET}/${RUST_BUILD_TYPE}/librerun_c.a)
elseif(UNIX) # if(LINUX) # CMake 3.25
    set(RERUN_C_BUILD_ARTIFACT ${CARGO_TARGET_DIR}/$ENV{CARGO_BUILD_TARGET}/${RUST_BUILD_TYPE}/librerun_c.a)
elseif(WIN32)
    set(RERUN_C_BUILD_ARTIFACT ${CARGO_TARGET_DIR}/$ENV{CARGO_BUILD_TARGET}/${RUST_BUILD_TYPE}/rerun_c.lib)
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
    COMMAND cargo build ${CARGO_BUILD_FLAG} -p rerun_c
    COMMENT "Building rerun_c from source"
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
)

# In CMake you can't depend on an output file directly. We have to wrap this in a target that rerun_c then depends on.
add_custom_target(rerun_c_build DEPENDS "${RERUN_C_BUILD_ARTIFACT}")
add_dependencies(rerun_c rerun_c_build)
