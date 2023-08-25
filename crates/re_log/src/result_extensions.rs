pub trait ResultExt<T, E> {
    /// Logs an error if the result is an error and returns the result.
    fn ok_or_log_error(self) -> Option<T>
    where
        E: std::fmt::Display;

    /// Unwraps in debug builds otherwise logs an error if the result is an error and returns the result.
    fn unwrap_debug_or_log_error(self) -> Option<T>
    where
        E: std::fmt::Display + std::fmt::Debug;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    #[track_caller]
    fn ok_or_log_error(self) -> Option<T>
    where
        E: std::fmt::Display,
    {
        match self {
            Ok(t) => Some(t),
            Err(err) => {
                let loc = std::panic::Location::caller();
                log::error!("{}:{} {err}", loc.file(), loc.line());
                None
            }
        }
    }

    #[track_caller]
    fn unwrap_debug_or_log_error(self) -> Option<T>
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        if cfg!(debug_assertions) {
            Some(self.unwrap())
        } else {
            self.ok_or_log_error()
        }
    }
}
