#pragma once

// If the library is not compiled on Windows, RERUN_SDK_EXPORT is defined as empty macro.
#ifndef _MSC_VER
#define RERUN_SDK_EXPORT
#else
// If rerun_sdk is compiled as shared on Windows, RERUN_SDK_EXPORT
// is __declspec(dllexport) for the compilation unit that are part
// of the library, and __declspec(dllimport) for compilation units
// that link to the library.
#ifdef RERUN_SDK_COMPILED_AS_SHARED_LIBRARY
// rerun_sdk_EXPORTS is defined by CMake itself when compiling a shared
// library, see https://cmake.org/cmake/help/latest/prop_tgt/DEFINE_SYMBOL.html
#ifdef rerun_sdk_EXPORTS
// We are building this library.
#define RERUN_SDK_EXPORT __declspec(dllexport)
#else
// We are using this library.
#define RERUN_SDK_EXPORT __declspec(dllimport)
#endif
#else
// If rerun_sdk is compiled as static on Windows, RERUN_SDK_EXPORT is defined as an empty macro.
#define RERUN_SDK_EXPORT
#endif
#endif
