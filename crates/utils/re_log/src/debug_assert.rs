/// Like [`std::debug_assert!`], but prefixes any failure message with "DEBUG ASSERT".
///
/// We use `if cfg!(…)` instead of `#[cfg(…)]` so that the code is still
/// compiled in release builds, to avoid unused variable warnings.
#[macro_export]
macro_rules! debug_assert {
    ($cond:expr $(,)?) => {
        if cfg!(debug_assertions) && !$cond {
            ::core::panic!(
                "DEBUG ASSERT: assertion failed: {}",
                ::core::stringify!($cond),
            );
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if cfg!(debug_assertions) && !$cond {
            ::core::panic!("DEBUG ASSERT: {}", ::core::format_args!($($arg)+));
        }
    };
}

/// Like [`std::debug_assert_eq!`], but prefixes any failure message with "DEBUG ASSERT".
///
/// We use `if cfg!(…)` instead of `#[cfg(…)]` so that the code is still
/// compiled in release builds, to avoid unused variable warnings.
#[macro_export]
macro_rules! debug_assert_eq {
    ($left:expr, $right:expr $(,)?) => {
        match (&$left, &$right) {
            (left, right) => {
                if cfg!(debug_assertions) && *left != *right {
                    ::core::panic!(
                        "DEBUG ASSERT: assertion `left == right` failed\n  left: {left:?}\n right: {right:?}",
                    );
                }
            }
        }
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        match (&$left, &$right) {
            (left, right) => {
                if cfg!(debug_assertions) && *left != *right {
                    ::core::panic!(
                        "DEBUG ASSERT: assertion `left == right` failed: {}\n  left: {left:?}\n right: {right:?}",
                        ::core::format_args!($($arg)+),
                    );
                }
            }
        }
    };
}

/// Panics in debug builds with a "DEBUG PANIC: " prefix.
///
/// Use this instead of `debug_assert!(false, …)`.
#[macro_export]
macro_rules! debug_panic {
    ($($arg:tt)+) => {
        if cfg!(debug_assertions) {
            ::core::panic!("DEBUG PANIC: {}", ::core::format_args!($($arg)+));
        }
    };
}
