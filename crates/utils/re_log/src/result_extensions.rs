pub trait ResultExt<T, E> {
    /// Logs an error if the result is an error and returns the result.
    fn ok_or_log_error(self) -> Option<T>;

    /// Logs an error if the result is an error and returns the result, but only once.
    fn ok_or_log_error_once(self) -> Option<T>;

    /// Log a warning if there is an `Err`, but only log the exact same message once.
    fn warn_on_err_once(self, msg: impl std::fmt::Display) -> Option<T>;

    /// Unwraps in debug builds otherwise logs an error if the result is an error and returns the result.
    fn unwrap_debug_or_log_error(self) -> Option<T>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::fmt::Display,
{
    #[track_caller]
    fn ok_or_log_error(self) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(err) => {
                let loc = std::panic::Location::caller();
                let (file, line) = (loc.file(), loc.line());
                log::error!("{file}:{line} {err}");
                None
            }
        }
    }

    #[track_caller]
    fn ok_or_log_error_once(self) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(err) => {
                let loc = std::panic::Location::caller();
                let (file, line) = (loc.file(), loc.line());
                crate::error_once!("{file}:{line} {err}");
                None
            }
        }
    }

    /// Log a warning if there is an `Err`, but only log the exact same message once.
    #[track_caller]
    fn warn_on_err_once(self, msg: impl std::fmt::Display) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(err) => {
                let loc = std::panic::Location::caller();
                let (file, line) = (loc.file(), loc.line());
                crate::warn_once!("{file}:{line} {msg}: {err}");
                None
            }
        }
    }

    #[track_caller]
    fn unwrap_debug_or_log_error(self) -> Option<T> {
        if cfg!(debug_assertions) {
            #[expect(clippy::panic)]
            match self {
                Ok(value) => Some(value),
                Err(err) => {
                    let loc = std::panic::Location::caller();
                    let (file, line) = (loc.file(), loc.line());
                    panic!("{file}:{line} {err}");
                }
            }
        } else {
            self.ok_or_log_error()
        }
    }
}
