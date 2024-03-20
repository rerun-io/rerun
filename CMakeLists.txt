cmake_minimum_required(VERSION 3.16...3.27)

project(rerun_cpp_proj LANGUAGES CXX)

# ------------------------------------------------------------------------------

# Environment driven hooks so we can configure CMake configure even if its buried deep inside a pixi task.
if(DEFINED ENV{RERUN_WERROR})
    message(STATUS "Compiling with warnings as errors since RERUN_WERROR is set.")
    set(CMAKE_COMPILE_WARNING_AS_ERROR ON)
endif()

if(DEFINED ENV{RERUN_USE_ASAN})
    message(STATUS "Compiling with address sanatizer ('asan') since RERUN_USE_ASAN is set.")
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

        # CMAKE_COMPILE_WARNING_AS_ERROR is only directly supported starting in CMake `3.24`
        # https://cmake.org/cmake/help/latest/prop_tgt/COMPILE_WARNING_AS_ERROR.html
        if(CMAKE_COMPILE_WARNING_AS_ERROR)
            target_compile_options(${target} PRIVATE /WX)
        endif()

        if(RERUN_USE_ASAN)
            message(WARNING "Rerun C++ build does not support address sanatizer ('asan') on Windows. Ignoring RERUN_USE_ASAN.")
        endif()
    else()
        # Enabled warnings.
        target_compile_options(${target} PRIVATE
            -Wall
            -Wcast-align
            -Wcast-qual
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
            # Turn on the address sanatizer ("asan"):
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

# Arrow requires a C++17 compliant compiler.
if(NOT DEFINED CMAKE_CXX_STANDARD)
    set(CMAKE_CXX_STANDARD 17)
endif()

# Signal to all our build scripts that we're inside the Rerun repository.
set(RERUN_REPOSITORY YES)

# Set version number.
# Read it from the Rust toml with a regex so we don't have another place to maintain!
file(READ "Cargo.toml" CARGO_TOML)
string(REGEX MATCH "\nversion = \"([a-z0-9\.\+-]+)\"\n" _ ${CARGO_TOML})
set(RERUN_VERSION ${CMAKE_MATCH_1})
message(STATUS "Rerun version ${RERUN_VERSION}")

# ------------------------------------------------------------------------------
# Loguru logging library (https://github.com/emilk/loguru):
set(CMAKE_DL_LIBS "dl") # Required by Loguru for backtraces

# Loguru, see https://github.com/emilk/loguru/blob/master/loguru_cmake_example/CMakeLists.txt
include(FetchContent)
FetchContent_Declare(LoguruGitRepo
    GIT_REPOSITORY "https://github.com/emilk/loguru" # can be a filesystem path
    GIT_TAG "master"
)
FetchContent_MakeAvailable(LoguruGitRepo) # defines target 'loguru::loguru'

# Set any loguru compile-time flags before calling MakeAvailable()
# Stacktraces are not yet supported on Windows.
if(NOT WIN32)
    set(LOGURU_STACKTRACES 1)
endif()

# ------------------------------------------------------------------------------
add_subdirectory(crates/rerun_c) # The Rerun C SDK library, must be included before the C++ SDK.
add_subdirectory(rerun_cpp) # The Rerun C++ SDK library.
add_subdirectory(examples/cpp)
add_subdirectory(tests/cpp)
add_subdirectory(docs/snippets)
