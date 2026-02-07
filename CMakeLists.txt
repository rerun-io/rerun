cmake_minimum_required(VERSION 3.16...3.27)

project(rerun_cpp_proj LANGUAGES CXX)

# ------------------------------------------------------------------------------

# Environment driven hooks so we can configure CMake configure even if its buried deep inside a pixi task.
if(DEFINED ENV{RERUN_WERROR})
    message(STATUS "Compiling with warnings as errors since RERUN_WERROR is set.")
    set(CMAKE_COMPILE_WARNING_AS_ERROR ON)
endif()

if(DEFINED ENV{RERUN_USE_ASAN})
    message(STATUS "Compiling with address sanitizer ('asan') since RERUN_USE_ASAN is set.")
    set(RERUN_USE_ASAN ON)
endif()

if(DEFINED ENV{RERUN_SET_CXX_VERSION})
    message(STATUS "Overwriting CMAKE_CXX_STANDARD to $ENV{RERUN_SET_CXX_VERSION} since RERUN_SET_CXX_VERSION is set accordingly.")
    set(CMAKE_CXX_STANDARD $ENV{RERUN_SET_CXX_VERSION})
endif()

# ------------------------------------------------------------------------------
function(rerun_strict_warning_settings target)
    if(MSVC)
        # TODO(andreas): Try to enable /Wall
        target_compile_options(${target} PRIVATE /W4)

        target_compile_options(${target} PRIVATE /we4996) # Using deprecated functions is an error

        if(BUILD_SHARED_LIBS)
            # If we are building as shared libs, we are going to have to disable the C4251
            # warning, as it would trigger for any datatype derived from a STL class
            # See also https://github.com/protocolbuffers/protobuf/blob/v26.1/cmake/README.md#notes-on-compiler-warnings
            # We need also to make it public, otherwise downstream code will be flooded by c4251 warnings
            target_compile_options(${target} PUBLIC /wd4251)
        endif()

        # CMAKE_COMPILE_WARNING_AS_ERROR is only directly supported starting in CMake `3.24`
        # https://cmake.org/cmake/help/latest/prop_tgt/COMPILE_WARNING_AS_ERROR.html
        if(CMAKE_COMPILE_WARNING_AS_ERROR)
            target_compile_options(${target} PRIVATE /WX
                /w15038 # Initialization order. https://learn.microsoft.com/en-us/cpp/error-messages/compiler-warnings/c5038
            )
        endif()

        if(RERUN_USE_ASAN)
            message(WARNING "Rerun C++ build does not support address sanitizer ('asan') on Windows. Ignoring RERUN_USE_ASAN.")
        endif()
    else()
        # Enabled warnings.
        target_compile_options(${target} PRIVATE
            -Wall
            -Wcast-align
            -Wcast-qual
            -Werror=deprecated-declarations
            -Wextra
            -Wformat=2
            -Wmissing-include-dirs
            -Wnull-dereference
            -Wold-style-cast
            -Wpedantic
            -Wpointer-arith
            -Wshadow
            -Wswitch-enum
            -Wunreachable-code
            -Wvla
        )

        if(CMAKE_CXX_COMPILER_ID MATCHES "Clang") # match both "Clang" and "AppleClang"
            # TODO(emilk): enable some hardening flags from https://best.openssf.org/Compiler-Hardening-Guides/Compiler-Options-Hardening-Guide-for-C-and-C++.html
            target_compile_options(${target} PRIVATE
                -Wc++17-compat-pedantic
                -Wc++20-compat-pedantic
                -Wc99-extensions
                -Weverything
                -Wgnu
                -Wnon-gcc
                -Wpre-c2x-compat-pedantic
                -Wshadow-all

                # Turn off some warning that -Weverything turns on:
                -Wno-c++98-compat
                -Wno-c++98-compat-pedantic
                -Wno-covered-switch-default # We often add a `default:` case out of paranoia
                -Wno-ctad-maybe-unsupported
                -Wno-disabled-macro-expansion
                -Wno-documentation
                -Wno-documentation-unknown-command
                -Wno-double-promotion # float->double is nothing to whine about
                -Wno-exit-time-destructors
                -Wno-float-equal # comparing floats is fine
                -Wno-global-constructors
                -Wno-missing-prototypes
                -Wno-padded
                -Wno-reserved-id-macro
                -Wno-reserved-identifier
                -Wno-unreachable-code-break # TODO(emilk): remove this exception - we only need this because of codegen
                -Wno-unreachable-code-return # TODO(emilk): remove this exception - we only need this because of codegen
                -Wno-unused-macros
                -Wno-unsafe-buffer-usage # There's a few helper ctors that run into this.
                -Wno-unknown-warning-option # Otherwise older clang will complain about `-Wno-unsafe-buffer-usage`
            )
        endif()

        # CMAKE_COMPILE_WARNING_AS_ERROR is only directly supported starting in CMake `3.24`
        # https://cmake.org/cmake/help/latest/prop_tgt/COMPILE_WARNING_AS_ERROR.html
        if(CMAKE_COMPILE_WARNING_AS_ERROR)
            target_compile_options(${target} PRIVATE -Werror)
        endif()

        if(CMAKE_BUILD_TYPE STREQUAL "Debug")
            # Improve stack traces:
            target_compile_options(${target} PRIVATE -g -fno-omit-frame-pointer -mno-omit-leaf-frame-pointer -fno-optimize-sibling-calls)
        endif()

        if(RERUN_USE_ASAN)
            # Turn on the address sanitizer ("asan"):
            target_compile_options(${target} PRIVATE -fsanitize=address)
            target_link_options(${target} PRIVATE -fsanitize=address)
        endif()
    endif()
endfunction()

# Use makefiles on linux, otherwise it might use Ninja which isn't installed by default.
if(NOT DEFINED CMAKE_GENERATOR AND UNIX)
    set(CMAKE_GENERATOR "Unix Makefiles")
endif()

# If using MSVC, always enable multi-process compiling for all targets.
# Note that this setting is repeated for rerun_sdk's CMakeLists.txt since it should also work stand-alone.
if(MSVC)
    add_compile_options("/MP")
endif()

# Signal to all our build scripts that we're inside the Rerun repository.
set(RERUN_REPOSITORY YES)

# ------------------------------------------------------------------------------
# Loguru logging library (https://github.com/emilk/loguru):
set(CMAKE_DL_LIBS "dl") # Required by Loguru for backtraces

# Loguru, see https://github.com/emilk/loguru/blob/4adaa185883e3c04da25913579c451d3c32cfac1/loguru_cmake_example/CMakeLists.txt
include(FetchContent)
FetchContent_Declare(LoguruGitRepo
    GIT_REPOSITORY "https://github.com/emilk/loguru" # can be a filesystem path
    GIT_TAG "master"
)

# Loguru does not support being build with BUILD_SHARED_LIBS=ON on Windows
# so we always compile it with BUILD_SHARED_LIBS=OFF on Windows
if(MSVC AND BUILD_SHARED_LIBS)
    set(BUILD_SHARED_LIBS_RERUN_SDK ${BUILD_SHARED_LIBS})
    set(BUILD_SHARED_LIBS OFF)
endif()

FetchContent_MakeAvailable(LoguruGitRepo) # defines target 'loguru::loguru'

if(MSVC AND BUILD_SHARED_LIBS_RERUN_SDK)
    set(BUILD_SHARED_LIBS ${BUILD_SHARED_LIBS_RERUN_SDK})
endif()

# Set any loguru compile-time flags before calling MakeAvailable()
# Stacktraces are not yet supported on Windows.
if(NOT WIN32)
    set(LOGURU_STACKTRACES 1)
endif()

# ------------------------------------------------------------------------------
add_subdirectory(crates/top/rerun_c) # The Rerun C SDK library, must be included before the C++ SDK.
add_subdirectory(rerun_cpp) # The Rerun C++ SDK library.
add_subdirectory(examples/cpp)
add_subdirectory(tests/cpp)
add_subdirectory(docs/snippets)

# ------------------------------------------------------------------------------
# Setup an 'all_targets' target that depends on all targets in the project.
#
# CMake already has an `all` (lowercase!) target as the default,
# but it does NOT work with Visual Studio and XCode.
# See https://cmake.org/cmake/help/latest/guide/user-interaction/index.html#selecting-a-target

# Collect all currently added targets in all subdirectories
#
# Via: https://stackoverflow.com/a/60232044
#
# Parameters:
# - _result the list containing all found targets
# - _dir root directory to start looking from
function(get_all_targets _result _dir)
    get_property(_subdirs DIRECTORY "${_dir}" PROPERTY SUBDIRECTORIES)

    foreach(_subdir IN LISTS _subdirs)
        get_all_targets(${_result} "${_subdir}")
    endforeach()

    get_directory_property(_sub_targets DIRECTORY "${_dir}" BUILDSYSTEM_TARGETS)
    set(${_result} ${${_result}} ${_sub_targets} PARENT_SCOPE)
endfunction()

get_all_targets(all_targets ${CMAKE_CURRENT_SOURCE_DIR})

add_custom_target(ALL DEPENDS ${all_targets})

foreach(target IN LISTS all_targets_list)
    add_dependencies(ALL ${target})
endforeach()
