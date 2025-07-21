//! The Rerun Python Log SDK.
//!
//! This provides bindings between Python and Rust.
//! It compiles into a Python wheel using <https://github.com/PyO3/pyo3>.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// NOTE: The SDK currently allocates *a lot*, so much in fact that adding accounting around
// allocations yields a lot of overhead.
//
// use re_memory::AccountingAllocator;
//
// #[global_allocator]
// static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
//     AccountingAllocator::new(mimalloc::MiMalloc);

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod arrow;
mod catalog;
mod dataframe;
mod python_bridge;
mod utils;
mod video;
mod viewer;
