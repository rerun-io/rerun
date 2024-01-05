pub mod components;
#[cfg(not(target_arch = "wasm32"))]
pub mod validation;
#[cfg(not(target_arch = "wasm32"))]
mod validation_gen;

#[cfg(not(target_arch = "wasm32"))]
pub use validation_gen::is_valid_blueprint;
