#ifndef RR_DEPRECATED
// Mark as deprecated in C
#if defined(__GNUC__) || defined(__clang__)
#define RR_DEPRECATED(msg) __attribute__((deprecated))
#elif defined(_MSC_VER)
#define RR_DEPRECATED(msg) __declspec(deprecated(msg))
#else
#define RR_DEPRECATED(msg)
#endif // define checks
#endif // RR_DEPRECATED
