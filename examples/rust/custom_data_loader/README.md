---
title: Custom data-loader
tags: [data-loader, extension]
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/custom_data_loader/src/main.rs?speculative-link
---

<picture>
  <img src="https://static.rerun.io/custom_data_loader/e44aadfa02fade5a3cf5d7cbdd6e0bf65d9f6446/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_data_loader/e44aadfa02fade5a3cf5d7cbdd6e0bf65d9f6446/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_data_loader/e44aadfa02fade5a3cf5d7cbdd6e0bf65d9f6446/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_data_loader/e44aadfa02fade5a3cf5d7cbdd6e0bf65d9f6446/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_data_loader/e44aadfa02fade5a3cf5d7cbdd6e0bf65d9f6446/1200w.png">
</picture>

This example demonstrates how to implement and register a `DataLoader` into the Rerun Viewer in order to add support for loading arbitrary files.

Usage:
```sh
$ cargo r -p custom_data_loader -- path/to/some/file
```
