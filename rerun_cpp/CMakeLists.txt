# ------------------------------------------------------------------------------
# Rerun C++ SDK
#
# For more information check README.md
# ------------------------------------------------------------------------------

cmake_minimum_required(VERSION 3.16...3.27)

project(rerun_sdk LANGUAGES CXX)

message("Compiler: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION} (${CMAKE_CXX_COMPILER})")

set(RERUN_CPP_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/src" CACHE PATH "Rerun include & source directory")

file(GLOB_RECURSE rerun_sdk_SRC CONFIGURE_DEPENDS
    "${RERUN_CPP_SOURCE_DIR}/*.hpp"
    "${RERUN_CPP_SOURCE_DIR}/*.cpp"
)

add_library(rerun_sdk ${rerun_sdk_SRC} ${rerun_sdk_PUBLIC_HEADER})

# Make sure the compiler can find include files for rerun when other libraries or executables link to rerun.
# Mark include directories as system includes to suppress warnings from them.
target_include_directories(rerun_sdk PUBLIC
    $<BUILD_INTERFACE:${RERUN_CPP_SOURCE_DIR}>
    $<INSTALL_INTERFACE:include>
)

# Rerun needs at least C++17.
target_compile_features(rerun_sdk PUBLIC cxx_std_17)

# Do multithreaded compiling on MSVC.
if(MSVC)
    target_compile_options(rerun_sdk PRIVATE "/MP")
endif()

# Set default warning settings if defined.
if(COMMAND rerun_strict_warning_settings)
    message("Building Rerun C++ SDK with strict compilation warnings.")
    rerun_strict_warning_settings(rerun_sdk)
endif()

# ------------------------------------------------------------------------------
# Setup rerun_c dependency if it wasn't set up already.
if(NOT TARGET rerun_c)
    add_library(rerun_c STATIC IMPORTED GLOBAL)

    # Inside the repo build ourselves, otherwise default to a local `lib` folder.
    set(RERUN_C_DEFAULT_LIB_DIR ${CMAKE_CURRENT_SOURCE_DIR}/lib/)

    if(APPLE)
        if("${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "x86_64")
            message(WARNING "macOS x86 is officially unsupported by Rerun.")
            set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__macos_x64.a)
        elseif("${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "arm64" OR "${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "aarch64")
            set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__macos_arm64.a)
        else()
            message(WARNING "Unknown architecture ${CMAKE_SYSTEM_PROCESSOR}, can't find rerun_c library.")
        endif()
    elseif(UNIX) # if(LINUX) # CMake 3.25
        if("${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "x86_64")
            set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__linux_x64.a)
        elseif("${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "arm64" OR "${CMAKE_SYSTEM_PROCESSOR}" STREQUAL "aarch64")
            set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/librerun_c__linux_arm64.a)
        else()
            message(WARNING "Unknown architecture ${CMAKE_SYSTEM_PROCESSOR}, can't find rerun_c library.")
        endif()
    elseif(WIN32)
        # TODO(andreas): Arm support.
        set(RERUN_C_LIB_DEFAULT ${RERUN_C_DEFAULT_LIB_DIR}/rerun_c__win_x64.lib)
    else()
        message(WARNING "Unsupported platform ${RERUN_C_LIB_DEFAULT}, can't find rerun_c library.")
    endif()

    set(RERUN_C_LIB ${RERUN_C_LIB_DEFAULT} CACHE PATH "\
        Where to find the rerun_c library.\n\
        If not specified, a local rerun_c with the current system architecture will be used."
    )

    if("${RERUN_C_LIB}" STREQUAL "")
        message(FATAL_ERROR "RERUN_C_LIB is not set.")
    endif()

    set_target_properties(rerun_c PROPERTIES IMPORTED_LOCATION ${RERUN_C_LIB})
endif()

if(APPLE)
    target_link_libraries(rerun_c INTERFACE "-framework CoreFoundation" "-framework IOKit" "-framework Security")
elseif(UNIX) # if(LINUX) # CMake 3.25
    target_link_libraries(rerun_c INTERFACE "-lm -ldl -pthread")
elseif(WIN32)
    target_link_libraries(rerun_c INTERFACE
        Crypt32
        Iphlpapi
        Ncrypt
        Netapi32
        ntdll
        Pdh
        PowrProf
        Psapi
        Secur32
        Userenv
        ws2_32
    )
endif()

target_link_libraries(rerun_sdk PRIVATE rerun_c)

# -----------------------------------------------------------------------------
# Arrow dependency.
# This makes the setup a lot easier on Windows where we otherwise need to put Arrow.dll either in path or copy it with the executable.
option(RERUN_DOWNLOAD_AND_BUILD_ARROW "If enabled, arrow will be added as an external project and built with the minimal set required by the Rerun C++ SDK" ON)

if (NOT RERUN_DOWNLOAD_AND_BUILD_ARROW)
    find_package(Arrow REQUIRED)
endif()

if (ARROW_BUILD_SHARED)
    set(RERUN_ARROW_LINK_SHARED_DEFAULT ON)
else()
    set(RERUN_ARROW_LINK_SHARED_DEFAULT OFF)
endif()

option(RERUN_ARROW_LINK_SHARED "Link to the Arrow shared library." ${RERUN_ARROW_LINK_SHARED_DEFAULT})

if(RERUN_DOWNLOAD_AND_BUILD_ARROW)
    include(download_and_build_arrow.cmake)
    download_and_build_arrow() # populates `rerun_arrow_target`
else()
    if(RERUN_ARROW_LINK_SHARED)
        add_library(rerun_arrow_target ALIAS Arrow::arrow_shared)
    else()
        add_library(rerun_arrow_target ALIAS Arrow::arrow_static)
    endif()
endif()

target_link_libraries(rerun_sdk PRIVATE rerun_arrow_target)

if(MSVC AND BUILD_SHARED_LIBS)
    # This code is required by to support BUILD_SHARED_LIBS=ON on Windows
    # Differently from Linux/macOS, by default Windows does not support
    # exporting all symbols of a shared library, and instead requires
    # annotating manually each class that needs to be exported
    # The WINDOWS_EXPORT_ALL_SYMBOLS PROPERTY (set in the next line)
    # simply the process of having shared library on Windows, by emulating
    # in CMake the behavior of Linux/macOS. However, it does not cover
    # static variables.
    set_property(TARGET rerun_sdk PROPERTY WINDOWS_EXPORT_ALL_SYMBOLS ON)

    # For exporting static variables in shared libraries in Windows, it
    # is not possible to just use WINDOWS_EXPORT_ALL_SYMBOLS, we need instead
    # to manually annotate with the appropriate storage-class attributes
    # all static variables. The easiest way to do so is use GenerateExportHeader
    # module to generate a rerun_sdk_export.hpp header file that define the RERUN_SDK_EXPORT
    # macro, and add that macro to all static variables. The RERUN_SDK_EXPORT is defined
    # in src/rerun/rerun_sdk_export.hpp . The definition of the macro changes depending
    # of whether the library is compiled as static or shared, so for shared builds we
    # set the RERUN_SDK_COMPILED_AS_SHARED_LIBRARY to let the header know if the build
    # is a shared library one
    target_compile_definitions(rerun_sdk PUBLIC RERUN_SDK_COMPILED_AS_SHARED_LIBRARY)
endif()

# -----------------------------------------------------------------------------
# Installation.
set(RERUN_SDK_INSTALL_CMAKE_DIR "lib/cmake/rerun_sdk")

# Actual install setup.
install(TARGETS rerun_sdk
    EXPORT rerun_sdkTargets
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
    INCLUDES DESTINATION include
)

# Some public headers are shared by both the C++ and C sdk, so we install
# them with the C++ headers also if RERUN_INSTALL_RERUN_C is OFF
set(RERUN_PATTERN_HEADER_SHARED_BETWEEN_CXX_AND_C "sdk_info.h")

# Add all C++ headers to the install.
install(DIRECTORY "${RERUN_CPP_SOURCE_DIR}/" TYPE INCLUDE FILES_MATCHING PATTERN "*.hpp" PATTERN ${RERUN_PATTERN_HEADER_SHARED_BETWEEN_CXX_AND_C})

option(RERUN_INSTALL_RERUN_C "Install rerun_c file." ON)

# if rerun_sdk is a static library it is compulsory to install rerun_c
get_target_property(rerun_sdk_TYPE rerun_sdk TYPE)

if(rerun_sdk_TYPE STREQUAL "STATIC_LIBRARY" AND NOT RERUN_INSTALL_RERUN_C)
    message(FATAL_ERROR "It is not possible to disable RERUN_INSTALL_RERUN_C option if rerun_sdk is compiled as static library.")
endif()

if(RERUN_INSTALL_RERUN_C)
    # CMake doesn't allow installing imported targets which is why we need to add this as a file.
    get_target_property(RERUN_C_LIB_LOCATION rerun_c IMPORTED_LOCATION)
    install(DIRECTORY "${RERUN_CPP_SOURCE_DIR}/" TYPE INCLUDE FILES_MATCHING PATTERN "*.h" PATTERN ${RERUN_PATTERN_HEADER_SHARED_BETWEEN_CXX_AND_C} EXCLUDE)
    install(FILES ${RERUN_C_LIB_LOCATION} DESTINATION lib)
endif()

# Similarly, we bundle the arrow library that was used during the build.
# For the moment we only support this when building with the downloaded, static libarrow.
# Otherwise, a system install of arrow has to be used.
get_target_property(RERUN_ARROW_LIB_LOCATION rerun_arrow_target LOCATION)

if(RERUN_DOWNLOAD_AND_BUILD_ARROW AND NOT RERUN_ARROW_LINK_SHARED)
    get_target_property(RERUN_ARROW_LIBRARY_FILE rerun_arrow_target LOCATION)
    get_target_property(RERUN_ARROW_BUNDLED_DEPENDENCIES_FILE arrow_targetBundledDeps LOCATION)
    install(FILES
        ${RERUN_ARROW_LIBRARY_FILE}
        ${RERUN_ARROW_BUNDLED_DEPENDENCIES_FILE}
        DESTINATION lib
    )
endif()

# Export the target to a script.
install(EXPORT rerun_sdkTargets
    FILE rerun_sdkTargets.cmake
    DESTINATION ${RERUN_SDK_INSTALL_CMAKE_DIR}
)

include(CMakePackageConfigHelpers)

# Extract the version from rerun.h.
# Intentionally only grab major.minor.patch, not the full version, since version file can't handle it otherwise.
file(READ "${CMAKE_CURRENT_SOURCE_DIR}/src/rerun/c/sdk_info.h" RERUN_H_CONTENTS)
string(REGEX MATCH "\n#define RERUN_SDK_HEADER_VERSION \"([0-9]+.[0-9]+.[0-9]+)" _ ${RERUN_H_CONTENTS})
set(RERUN_INSTALL_VERSION ${CMAKE_MATCH_1})
message(STATUS "Rerun SDK install version: ${RERUN_INSTALL_VERSION}")

# Package config file, so find_package(rerun_sdk) produces a target.
configure_package_config_file(${CMAKE_CURRENT_SOURCE_DIR}/Config.cmake.in
    "${CMAKE_CURRENT_BINARY_DIR}/rerun_sdkConfig.cmake" # file needs to follow convention of find_package.
    INSTALL_DESTINATION ${RERUN_SDK_INSTALL_CMAKE_DIR}
    NO_SET_AND_CHECK_MACRO
    NO_CHECK_REQUIRED_COMPONENTS_MACRO
)

# Version file for find_package.
write_basic_package_version_file(
    ${CMAKE_CURRENT_BINARY_DIR}/rerun_sdkConfigVersion.cmake
    VERSION ${RERUN_INSTALL_VERSION}
    COMPATIBILITY ExactVersion
)

# Add the find_package dependent files.
install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/rerun_sdkConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/rerun_sdkConfigVersion.cmake"
    DESTINATION ${RERUN_SDK_INSTALL_CMAKE_DIR}
)

# -----------------------------------------------------------------------------
# Add tests if they exist (they are not part of the distribution zip).
# Has direct dependency to arrow, so needs to happen last.
if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/tests)
    add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/tests)
endif()
