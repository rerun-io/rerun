# Builds rerun_c from source.
#
# This works only within a checkout of the Rerun repository, since the Rerun C rust source
# is not part of the Rerun C++ SDK distribution bundle (instead, pre-built libraries are provided).
function (build_rerun_c OUT_C_LIB)
    # TODO(andreas): use add_custom_command instead so this runs at build time! https://cmake.org/cmake/help/latest/command/add_custom_command.html#command:add_custom_command
    execute_process(COMMAND cargo build --release -p rerun_c RESULT_VARIABLE ret) # We link against this, so must be up-to-date

    # execute process doesn't fail if the process fails.
    # `COMMAND_ERROR_IS_FATAL ANY` parameter fixes this but is only available in CMake 3.19
    if(NOT(ret EQUAL "0"))
        message(FATAL_ERROR "Failed to build rerun_c.")
    endif()

    # Overwrite where to find rerun_c library.
    if(APPLE)
        set(${OUT_C_LIB} ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    elseif(UNIX) # if(LINUX) # CMake 3.25
        set(${OUT_C_LIB} ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    elseif(WIN32)
        set(${OUT_C_LIB} ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/rerun_c.lib)
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
        NEWLINE_STYLE LF # Specify line endings, otherwise CMake wants to change them on Windows.
    )
endfunction()
