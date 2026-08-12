/// The formatted messages emitted by one `*_once!` macro call site.
/// Repeated messages are suppressed, while each distinct message is emitted once.
#[doc(hidden)]
#[derive(Default)]
pub struct LogOnceSet(parking_lot::Mutex<std::collections::BTreeSet<String>>);

impl LogOnceSet {
    /// Insert a message and return whether it was not already present.
    #[doc(hidden)]
    pub fn insert(&self, message: String) -> bool {
        self.0.lock().insert(message)
    }
}

/// Keeps a lazily initialized, mutex-protected message set at each `*_once!` call site.
/// Emits each unseen formatted message through [`event!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __log_once {
    ($level:expr, target: $target:expr, $($arg:tt)+) => {{
        static SEEN_MESSAGES: std::sync::LazyLock<$crate::LogOnceSet> =
            std::sync::LazyLock::new($crate::LogOnceSet::default);

        let message = format!($($arg)+);
        if SEEN_MESSAGES.insert(message.clone()) {
            $crate::event!(target: $target, $level, "{message}");
        }
    }};
    ($level:expr, $($arg:tt)+) => {{
        static SEEN_MESSAGES: std::sync::LazyLock<$crate::LogOnceSet> =
            std::sync::LazyLock::new($crate::LogOnceSet::default);

        let message = format!($($arg)+);
        if SEEN_MESSAGES.insert(message.clone()) {
            $crate::event!($level, "{message}");
        }
    }};
}

/// Like [`crate::trace!`], but logs each distinct message at most once per call site.
#[macro_export]
macro_rules! trace_once {
    ($($arg:tt)+) => {
        $crate::__log_once!($crate::Level::TRACE, $($arg)+)
    };
}

/// Like [`crate::debug!`], but logs each distinct message at most once per call site.
#[macro_export]
macro_rules! debug_once {
    ($($arg:tt)+) => {
        $crate::__log_once!($crate::Level::DEBUG, $($arg)+)
    };
}

/// Like [`crate::info!`], but logs each distinct message at most once per call site.
#[macro_export]
macro_rules! info_once {
    ($($arg:tt)+) => {
        $crate::__log_once!($crate::Level::INFO, $($arg)+)
    };
}

/// Like [`crate::warn!`], but logs each distinct message at most once per call site.
#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)+) => {
        $crate::__log_once!($crate::Level::WARN, $($arg)+)
    };
}

/// Like [`crate::error!`], but logs each distinct message at most once per call site.
#[macro_export]
macro_rules! error_once {
    ($($arg:tt)+) => {
        $crate::__log_once!($crate::Level::ERROR, $($arg)+)
    };
}

/// Log once at the given [`crate::Level`].
#[macro_export]
macro_rules! log_once {
    ($level:expr, $($arg:tt)+) => {
        match $level {
            $crate::Level::ERROR => $crate::error_once!($($arg)+),
            $crate::Level::WARN => $crate::warn_once!($($arg)+),
            $crate::Level::INFO => $crate::info_once!($($arg)+),
            $crate::Level::DEBUG => $crate::debug_once!($($arg)+),
            $crate::Level::TRACE => $crate::trace_once!($($arg)+),
        }
    };
}
