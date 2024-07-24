//! Examples for using 2D rendering.
//!
//! On the left is a 2D view, on the right a 3D view of the same scene.

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

mod framework;

#[cfg(target_arch = "wasm32")]
#[path = "./video/web.rs"]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    framework::start::<web::RenderVideo>();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    panic!("this demo is web-only")
}
