cmake_minimum_required(VERSION 3.16...3.27)

# NOTE: CMake docs strongly discourages using GLOB, and instead suggests
# manually listing all the files, like it's 1972.
# However, that won't work for use since we auto-generate the source tree.
# See https://cmake.org/cmake/help/latest/command/file.html#glob
file(GLOB_RECURSE rerun_sdk_SRC CONFIGURE_DEPENDS
    "src/*.hpp"
    "src/*.cpp"
)

add_library(rerun_sdk ${rerun_sdk_SRC})

# Make sure the compiler can find include files for rerun when other libraries or executables link to rerun:
target_include_directories(rerun_sdk PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/src)

# Rerun needs at least C++17.
set_target_properties(rerun_sdk PROPERTIES CXX_STANDARD 17)

# Do multithreaded compiling on MSVC.
if(MSVC)
    target_compile_options(rerun_sdk PRIVATE "/MP")
endif()

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
        NEWLINE_STYLE LF # Specify line endings, otherwise CMake wants to change them on Windows.
    )
endif()

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
        set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/rerun_c__win_x64.lib)
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

# For cleanliness, create a rerun_c target with the correct dependencies.
# This allows other targets to depend on rerun_c directly if they need to.
add_library(rerun_c STATIC IMPORTED)
set_target_properties(rerun_c PROPERTIES IMPORTED_LOCATION ${RERUN_C_LIB})

if(APPLE)
    target_link_libraries(rerun_c INTERFACE "-framework CoreFoundation" "-framework IOKit")
elseif(UNIX) # if(LINUX) # CMake 3.25
    target_link_libraries(rerun_c INTERFACE "-lm -ldl -pthread")
elseif(WIN32)
    target_link_libraries(rerun_c INTERFACE ws2_32.dll Bcrypt.dll Userenv.dll ntdll.dll)
endif()

target_link_libraries(rerun_sdk PRIVATE rerun_c)

# -----------------------------------------------------------------------------
# Arrow:
option(RERUN_ARROW_LINK_SHARED "Link to the Arrow shared library" ON)
option(RERUN_ARROW_EXTERNAL_PROJECT "If enabled, arrow will be added as an external project and built with the minimal set required by the Rerun C++ SDK" ON)

if(RERUN_ARROW_EXTERNAL_PROJECT)
    include(ExternalProject)

    set(ARROW_DOWNLOAD_PATH ${CMAKE_BINARY_DIR}/arrow)

    if(RERUN_ARROW_LINK_SHARED)
        if(APPLE)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.dylib)
        elseif(UNIX) # if(LINUX) # CMake 3.25
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.so)
        elseif(WIN32)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/bin/arrow.dll)
        else()
            message(FATAL_ERROR "Unsupported platform.")
        endif()
    else()
        if(APPLE)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.a)
        elseif(UNIX) # if(LINUX) # CMake 3.25
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.a)
        elseif(WIN32)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/arrow_static.lib)
        else()
            message(FATAL_ERROR "Unsupported platform.")
        endif()
    endif()

    # Enable multithreaded compiling of Arrow on MSVC.
    if(MSVC)
        set(DARROW_CXXFLAGS "/MP")
    else()
        set(DARROW_CXXFLAGS "")
    endif()

    ExternalProject_Add(
        arrow_cpp
        PREFIX ${ARROW_DOWNLOAD_PATH}
        GIT_REPOSITORY https://github.com/apache/arrow.git
        GIT_TAG apache-arrow-10.0.1
        GIT_SHALLOW true
        CMAKE_ARGS
        --preset ninja-debug-minimal
        -DARROW_IPC=ON
        -DARROW_BUILD_STATIC=${RERUN_ARROW_LINK_SHARED}
        -DCMAKE_INSTALL_PREFIX=${ARROW_DOWNLOAD_PATH}
        -DARROW_USE_ASAN=OFF
        -DARROW_USE_TSAN=OFF
        -DARROW_USE_UBSAN=OFF
        -Dxsimd_SOURCE=BUNDLED
        -DBOOST_SOURCE=BUNDLED
        -DARROW_BOOST_USE_SHARED=OFF
        -DARROW_CXXFLAGS=${DARROW_CXXFLAGS}
        SOURCE_SUBDIR cpp
        BUILD_BYPRODUCTS ${ARROW_LIBRARY_FILE}
    )

    # arrow_cpp target is not a library. Assemble one from it.
    if(RERUN_ARROW_LINK_SHARED)
        add_library(RerunArrowTarget SHARED IMPORTED)

        # For windows we need to know both the dll AND the import library.
        if(WIN32)
            set_target_properties(RerunArrowTarget PROPERTIES IMPORTED_IMPLIB ${ARROW_DOWNLOAD_PATH}/lib/arrow.lib)
        endif()
    else()
        add_library(RerunArrowTarget STATIC IMPORTED)
    endif()

    add_dependencies(RerunArrowTarget arrow_cpp)
    set_target_properties(RerunArrowTarget PROPERTIES
        IMPORTED_LOCATION ${ARROW_LIBRARY_FILE}
        INTERFACE_INCLUDE_DIRECTORIES ${ARROW_DOWNLOAD_PATH}/include
    )

    # Hack to propagate INTERFACE_INCLUDE_DIRECTORIES.
    # via https://stackoverflow.com/a/47358004
    file(MAKE_DIRECTORY ${ARROW_DOWNLOAD_PATH}/include)
else()
    find_package(Arrow REQUIRED)

    if(RERUN_ARROW_LINK_SHARED)
        message(STATUS "Arrow SO version: ${ARROW_FULL_SO_VERSION}")
        add_library(RerunArrowTarget ALIAS Arrow::arrow_shared)
    else()
        message(STATUS "Arrow version: ${ARROW_VERSION}")
        add_library(RerunArrowTarget ALIAS Arrow::arrow_static)
    endif()

    set(RERUN_ARROW_TARGET Arrow::arrow_shared)
endif()

target_link_libraries(rerun_sdk PRIVATE RerunArrowTarget)

# -----------------------------------------------------------------------------
# Add test subfolder if we're inside the Rerun repository
# (we don't ship them in our fetch-content bundle)
if(DEFINED RERUN_REPOSITORY)
    add_subdirectory(tests)
endif()
