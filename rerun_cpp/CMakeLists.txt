# ------------------------------------------------------------------------------
# Rerun C++ SDK
#
# For more information check README.md
# ------------------------------------------------------------------------------

cmake_minimum_required(VERSION 3.16...3.27)

message("Compiler: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION} (${CMAKE_CXX_COMPILER})")

# ------------------------------------------------------------------------------
# Base setup for rerun_sdk C++ library.

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
# Setup rerun_c dependency.
# Inside the repo build ourselves, otherwise default to a local `lib` folder.
if(DEFINED RERUN_REPOSITORY)
    include(build_rerun_c.cmake)
    build_rerun_c(RERUN_C_LIB_DEFAULT)
else()
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
# Arrow dependency.
# This makes the setup a lot easier on Windows where we otherwise need to put Arrow.dll either in path or copy it with the executable.
# Additionally reduces risk of picking up system libraries on Mac / Linux.
set(RERUN_ARROW_LINK_SHARED_DEFAULT OFF)
option(RERUN_ARROW_LINK_SHARED "Link to the Arrow shared library" ${RERUN_ARROW_LINK_SHARED_DEFAULT})
option(RERUN_DOWNLOAD_AND_BUILD_ARROW "If enabled, arrow will be added as an external project and built with the minimal set required by the Rerun C++ SDK" ON)

if(RERUN_DOWNLOAD_AND_BUILD_ARROW)
    include(download_and_build_arrow.cmake)
    download_and_build_arrow() # populates `rerun_arrow_target`
else()
    find_package(Arrow REQUIRED)

    if(RERUN_ARROW_LINK_SHARED)
        add_library(rerun_arrow_target ALIAS Arrow::arrow_shared)
    else()
        add_library(rerun_arrow_target ALIAS Arrow::arrow_static)
    endif()
endif()

target_link_libraries(rerun_sdk PRIVATE rerun_arrow_target)

# -----------------------------------------------------------------------------
# Additional settings for building from within the repo.
if(DEFINED RERUN_REPOSITORY)
    # Set of very string warnings.
    set_default_warning_settings(rerun_sdk)
    # Add tests (they are not part of the distribution zip).
    add_subdirectory(tests)
endif()
