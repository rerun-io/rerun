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
        self.0.clone().into()
    }
}
