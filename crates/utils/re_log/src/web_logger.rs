/// Implements [`log::Log`] to log messages to `console.log`, `console.warn`, etc.
pub struct WebLogger {
    filter: log::LevelFilter,
}

impl WebLogger {
    pub fn new(filter: log::LevelFilter) -> Self {
        Self { filter }
    }
}

impl log::Log for WebLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        crate::is_log_enabled(self.filter, metadata)
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

            // Using console.error causes crashes for unknown reason
            // https://github.com/emilk/egui/pull/2961
            // log::Level::Error => console::error(&msg),
            log::Level::Error => console::warn(&format!("ERROR: {msg}")),
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

        // Using console.error causes crashes for unknown reason
        // https://github.com/emilk/egui/pull/2961
        // /// `console.error`
        // #[wasm_bindgen(js_namespace = console)]
        // pub fn error(s: &str);
    }
}
