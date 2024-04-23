---
title: External data-loader example
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/external_data_loader/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/external_data_loader/src/main.rs
cpp: https://github.com/rerun-io/rerun/tree/latest/examples/cpp/external_data_loader/main.cpp
thumbnail: https://static.rerun.io/external_data_loader_rs/74eecea3b16fee7fab01045e3bfdd90ba6c59bc9/480w.png
---

<picture>
  <img src="https://static.rerun.io/external_data_loader_rs/74eecea3b16fee7fab01045e3bfdd90ba6c59bc9/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/external_data_loader_rs/74eecea3b16fee7fab01045e3bfdd90ba6c59bc9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/external_data_loader_rs/74eecea3b16fee7fab01045e3bfdd90ba6c59bc9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/external_data_loader_rs/74eecea3b16fee7fab01045e3bfdd90ba6c59bc9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/external_data_loader_rs/74eecea3b16fee7fab01045e3bfdd90ba6c59bc9/1200w.png">
</picture>

This is an example executable data-loader plugin for the Rerun Viewer.

It will log Rust source code files as markdown documents.
To try it out, install it in your $PATH (`cargo install --path . -f`), then open a Rust source file with Rerun (`rerun file.rs`).
