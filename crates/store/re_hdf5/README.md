# re_hdf5

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_hdf5.svg?speculative-link)](https://crates.io/crates/re_hdf5?speculative-link)
[![Documentation](https://docs.rs/re_hdf5/badge.svg?speculative-link)](https://docs.rs/re_hdf5?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Core HDF5-to-chunk loading logic for Rerun.

Reads an HDF5 file into a lazy stream of Rerun chunks:
each HDF5 group maps to an entity, each leaf dataset to a component, with a single file-wide timeline.
The backend is the pure-Rust `hdf5-pure` crate — no native libhdf5 dependency.
