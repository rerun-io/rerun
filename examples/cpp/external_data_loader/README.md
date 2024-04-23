---
title: External data-loader example
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/external_data_loader/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/external_data_loader/src/main.rs
cpp: https://github.com/rerun-io/rerun/tree/latest/examples/cpp/external_data_loader/main.cpp
thumbnail: https://static.rerun.io/external_data_loader_cpp/83cd3c2a322911cf597cf74aeda01c8fe83e275f/480w.png
---

<picture>
  <img src="https://static.rerun.io/external_data_loader_cpp/83cd3c2a322911cf597cf74aeda01c8fe83e275f/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/external_data_loader_cpp/83cd3c2a322911cf597cf74aeda01c8fe83e275f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/external_data_loader_cpp/83cd3c2a322911cf597cf74aeda01c8fe83e275f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/external_data_loader_cpp/83cd3c2a322911cf597cf74aeda01c8fe83e275f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/external_data_loader_cpp/83cd3c2a322911cf597cf74aeda01c8fe83e275f/1200w.png">
</picture>

This is an example executable data-loader plugin for the Rerun Viewer.

It will log C++ source code files as markdown documents.
To try it out, compile it and place it in your $PATH, then open a C++ source file with Rerun (`rerun file.cpp`).
