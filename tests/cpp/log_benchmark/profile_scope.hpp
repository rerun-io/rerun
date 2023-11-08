#pragma once

#include <chrono>
#include <cstdio>

/// Simplistic RAII scope for additional profiling.
///
/// All inlined on purpose.
/// Not threadsafe due to indentation!
class ProfileScope {
  public:
    // std::source_location would be nice here, but it's not widely enough supported
    // ProfileScope(const std::source_location& location = std::source_location::current())

    ProfileScope(const char* location)
        : _start(std::chrono::high_resolution_clock::now()), _location(location) {
        print_indent();
        printf("%s start …\n", _location);
        ++_indentation;
    }

    ~ProfileScope() {
        const auto end = std::chrono::high_resolution_clock::now();
        const auto duration =
            std::chrono::duration_cast<std::chrono::duration<double, std::milli>>(end - _start);
        --_indentation;
        print_indent();
        printf("%s end: %.2fms\n", _location, duration.count());
    }

  private:
    static void print_indent() {
        for (int i = 0; i < _indentation; ++i) {
            printf("--");
        }
        if (_indentation > 0) {
            printf(" ");
        }
    }

    std::chrono::high_resolution_clock::time_point _start;
    const char* _location;
    static int _indentation;
};

// Quick and dirty macro to profile a function.
#define PROFILE_FUNCTION() ProfileScope _function_profile_scope(__FUNCTION__)
