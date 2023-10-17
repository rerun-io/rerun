cmake_minimum_required(VERSION 3.16)

# NOTE: CMake docs strongly discourages using GLOB, and instead suggests
# manually listing all the files, like it's 1972.
# However, that won't work for use since we auto-generate the source tree.
# See https://cmake.org/cmake/help/latest/command/file.html#glob
file(GLOB_RECURSE rerun_sdk_SRC CONFIGURE_DEPENDS
    "src/*.hpp"
    "src/*.cpp"
)

add_library(rerun_sdk ${rerun_sdk_SRC})

# Rerun needs at least C++17.
set_property(TARGET rerun_sdk PROPERTY CXX_STANDARD 17)

# ------------------------------------------------------------------------------
# Rerun development setup only:
if(DEFINED RERUN_REPOSITORY)
    # TODO(andreas): use add_custom_command instead so this runs at build time! https://cmake.org/cmake/help/latest/command/add_custom_command.html#command:add_custom_command
    execute_process(COMMAND cargo build --release -p rerun_c RESULT_VARIABLE ret) # We link against this, so must be up-to-date

    # execute process doesn't fail if the process fails.
    # `COMMAND_ERROR_IS_FATAL ANY` parameter fixes this but is only available in CMake 3.19
    if(NOT(ret EQUAL "0"))
        message(FATAL_ERROR "Failed to build rerun_c.")
    endif()

    # Overwrite where to find rerun_c library.
    if(APPLE)
        set(RERUN_C_LIB_DEFAULT ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    elseif(UNIX) # if(LINUX) # CMake 3.25
        set(RERUN_C_LIB_DEFAULT ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    elseif(WIN32)
        set(RERUN_C_LIB_DEFAULT ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/rerun_c.lib)
    else()
        message(FATAL_ERROR "Unsupported platform.")
    endif()

    # Set very strict warning settings when we're testing the SDK.
    # We don't want to force this on any user!
    set_default_warning_settings(rerun_sdk)

    # Put `rerun.h` into the same place where it's on a user's machine and apply CMake variables like version number.
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/../crates/rerun_c/src/rerun.h"
        "${CMAKE_CURRENT_SOURCE_DIR}/src/rerun/c/rerun.h"
    )

    # Add tests!
    add_subdirectory(tests)
endif()

# ------------------------------------------------------------------------------

# Make sure the compiler can find include files for rerun
# when other libraries or executables link to rerun:
# Note that we have to use different path for within the repo and exporting the project:
target_include_directories(rerun_sdk PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/src)

# ------------------------------------------------------------------------------
# Setup rerun_c dependency:
if(NOT DEFINED RERUN_C_LIB_DEFAULT)
    set(RERUN_C_DEFAULT_LIB_DIR ${CMAKE_CURRENT_SOURCE_DIR}/lib/)

    if(APPLE)
        if("${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "x86_64")
            set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__macos_x64.a)
        elseif("${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "arm64")
            set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__macos_arm64.a)
        else()
            message(WARNING "Unknown architecture ${CMAKE_SYSTEM_PROCESSOR}, can't find rerun_c library.")
        endif()
    elseif(UNIX) # if(LINUX) # CMake 3.25
        # TODO(andreas): Arm support.
        set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__linux_x64.a)
    elseif(WIN32)
        # TODO(andreas): Arm support.
        set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__win_x64.lib)
    else()
        message(WARNING "Unsupported platform ${RERUN_C_LIB_DEFAULT}, can't find rerun_c library.")
    endif()
endif()

set(RERUN_C_LIB ${RERUN_C_LIB_DEFAULT} CACHE PATH "\
Where to find the rerun_c library.\n\
If not specified, a local rerun_c with the current system architecture will be used."
)

if("${RERUN_C_LIB}" STREQUAL "")
    message(FATAL_ERROR "RERUN_C_LIB is not set.")
endif()

target_link_libraries(rerun_sdk PRIVATE ${RERUN_C_LIB})

# -----------------------------------------------------------------------------
# Arrow:
option(RERUN_ARROW_LINK_SHARED "Link to the Arrow shared library" ON)

find_package(Arrow REQUIRED)

if(RERUN_ARROW_LINK_SHARED)
    message(STATUS "Arrow SO version: ${ARROW_FULL_SO_VERSION}")
    target_link_libraries(rerun_sdk PRIVATE Arrow::arrow_shared)
else()
    message(STATUS "Arrow version: ${ARROW_VERSION}")
    target_link_libraries(rerun_sdk PRIVATE Arrow::arrow_static)
endif()

# -----------------------------------------------------------------------------
# Other dependencies:
if(APPLE)
    target_link_libraries(rerun_sdk PRIVATE "-framework CoreFoundation" "-framework IOKit")
elseif(UNIX) # if(LINUX) # CMake 3.25
    target_link_libraries(rerun_sdk PRIVATE "-lm -ldl -pthread")
elseif(WIN32)
    target_link_libraries(rerun_sdk PUBLIC ws2_32.dll Bcrypt.dll Userenv.dll)
endif()
