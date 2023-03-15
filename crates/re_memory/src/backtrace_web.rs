use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: String);

    type Error;

    #[wasm_bindgen(constructor)]
    fn new() -> Error;

    #[wasm_bindgen(structural, method, getter)]
    fn stack(error: &Error) -> String;
}

#[derive(Hash)]
pub(crate) struct Backtrace(String);

impl Backtrace {
    pub fn new_unresolved() -> Self {
        Self(Error::new().stack())
    }

    pub fn format(&mut self) -> std::sync::Arc<str> {
        trim_backtrace(&self.0).to_owned().into()
    }
}

fn trim_backtrace(mut stack: &str) -> &str {
    let start_pattern = "__rust_alloc_zeroed";
    if let Some(start_offset) = stack.find(start_pattern) {
        if let Some(next_newline) = stack[start_offset..].find('\n') {
            stack = &stack[start_offset + next_newline + 1..];
        }
    }

    let end_pattern = "paint_and_schedule"; // normal eframe entry-point
    if let Some(end_offset) = stack.find(end_pattern) {
        if let Some(next_newline) = stack[end_offset..].find('\n') {
            stack = &stack[..end_offset + next_newline];
        }
    }

    stack
}
