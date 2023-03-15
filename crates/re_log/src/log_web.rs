/// Implements [`log::Log`] to log messages to `console.log`, `console.warn`, etc.
pub struct WebLogger {
    filter: log::LevelFilter,
}

impl WebLogger {
    pub fn new(filter: log::LevelFilter) -> Self {
        Self { filter }
    }

    /// Install this logger as the global logger.
    pub fn init(filter: log::LevelFilter) -> Result<(), log::SetLoggerError> {
        log::set_max_level(filter);
        log::set_boxed_logger(Box::new(Self::new(filter)))
    }
}

impl log::Log for WebLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if metadata.target().starts_with("wgpu") || metadata.target().starts_with("naga") {
            // TODO(emilk): remove once https://github.com/gfx-rs/wgpu/issues/3206 is fixed
            return metadata.level() <= log::LevelFilter::Warn;
        }

        metadata.level() <= self.filter
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let msg = if let (Some(file), Some(line)) = (record.file(), record.line()) {
            let file = crate::shorten_file_path(file);
            format!("[{}] {file}:{line}: {}", record.target(), record.args())
        } else {
            format!("[{}] {}", record.target(), record.args())
        };

        match record.level() {
            log::Level::Trace => console::trace(&msg),
            log::Level::Debug => console::debug(&msg),
            log::Level::Info => console::info(&msg),
            log::Level::Warn => console::warn(&msg),
            log::Level::Error => console::error(&msg),
        }
    }

    fn flush(&self) {}
}

/// js-bindings for console.log, console.warn, etc
mod console {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        /// `console.trace`
        #[wasm_bindgen(js_namespace = console)]
        pub fn trace(s: &str);

        /// `console.debug`
        #[wasm_bindgen(js_namespace = console)]
        pub fn debug(s: &str);

        /// `console.info`
        #[wasm_bindgen(js_namespace = console)]
        pub fn info(s: &str);

        /// `console.warn`
        #[wasm_bindgen(js_namespace = console)]
        pub fn warn(s: &str);

        /// `console.error`
        #[wasm_bindgen(js_namespace = console)]
        pub fn error(s: &str);
    }
}
