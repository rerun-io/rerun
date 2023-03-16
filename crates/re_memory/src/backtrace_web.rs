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
        trim_backtrace(&self.0).into()
    }
}

fn trim_backtrace(mut stack: &str) -> String {
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

    stack.split('\n').map(trim_line).collect::<String>()
}

/// Example input: `eframe::web::backend::AppRunner::paint::h584aff3234354fd5@http://127.0.0.1:9090/re_viewer.js line 366 > WebAssembly.instantiate:wasm-function[3352]:0x5d46b4`
///
/// Output: `eframe::web::backend::AppRunner::paint`
fn trim_line(line: &str) -> String {
    if let Some(last_double_colon) = line.rfind("::") {
        format!("{}\n", &line[0..last_double_colon])
    } else {
        format!("{line}\n")
    }
}
