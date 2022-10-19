fn main() {
    const CSS: &'static str = r#"
        body {
            overflow: hidden;
            margin: 0 !important;
            padding: 0 !important;
            height: 100%;
            width: 100%;
        }
    "#;
    cargo_run_wasm::run_wasm_with_css(CSS);
}
