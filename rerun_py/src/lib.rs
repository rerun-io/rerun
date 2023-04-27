//! The Rerun Python Log SDK.
//!
//! This provides bindings between Python and Rust.
//! It compiles into a Python wheel using <https://github.com/PyO3/pyo3>.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

mod arrow;
mod python_bridge;
