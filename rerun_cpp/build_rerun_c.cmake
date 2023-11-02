# Builds rerun_c from source.
#
# This works only within a checkout of the Rerun repository, since the Rerun C rust source
# is not part of the Rerun C++ SDK distribution bundle (instead, pre-built libraries are provided).
function (build_rerun_c rerun_c)
    if(APPLE)
        set(RERUN_C_BUILD_ARTIFACT ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    elseif(UNIX) # if(LINUX) # CMake 3.25
        set(RERUN_C_BUILD_ARTIFACT ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/librerun_c.a)
    elseif(WIN32)
        set(RERUN_C_BUILD_ARTIFACT ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/rerun_c.lib)
    else()
        message(FATAL_ERROR "Unsupported platform.")
    endif()

    # Just depend on all rust and toml files, it's hard to know which files exactly are relevant.
    file(GLOB_RECURSE RERUN_C_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/../crates/*.rs" "${CMAKE_CURRENT_SOURCE_DIR}/../crates/*.toml")
    add_custom_command(
        OUTPUT ${RERUN_C_BUILD_ARTIFACT}
        DEPENDS ${RERUN_C_SOURCES}
        COMMAND cargo build --release -p rerun_c
        COMMENT "Building rerun_c from source"
    )

    # In CMake you can't depend on an output file directly. We have to wrap this in a target that rerun_c then depends on.
    add_custom_target(rerun_c_build DEPENDS "${RERUN_C_BUILD_ARTIFACT}")
    add_dependencies(rerun_c rerun_c_build)
    set_target_properties(rerun_c PROPERTIES IMPORTED_LOCATION ${RERUN_C_BUILD_ARTIFACT})

    # Put `rerun.h` into the same place where it's on a user's machine and apply CMake variables like version number.
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/../crates/rerun_c/src/rerun.h"
        "${CMAKE_CURRENT_SOURCE_DIR}/src/rerun/c/rerun.h"
        NEWLINE_STYLE LF # Specify line endings, otherwise CMake wants to change them on Windows.
    )
endfunction()
