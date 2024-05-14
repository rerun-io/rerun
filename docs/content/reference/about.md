---
title: About
order: 100
---

To learn more about Rerun, the company, visit our Website at [https://www.rerun.io/](https://www.rerun.io/).

Code & License
--------------
The Rerun SDK & Viewer are open source, all code is available on [GitHub](https://github.com/rerun-io/rerun/) and open for contributions.
Licensing is permissive, the project is dual licensed under [MIT](https://github.com/rerun-io/rerun/blob/main/LICENSE-MIT) & [Apache 2.0](https://github.com/rerun-io/rerun/blob/main/LICENSE-APACHE).


Under the hood
--------------
The software is almost entirely written in [Rust](https://www.rust-lang.org/), a modern, fast and safe programming language.
If you're curious about why we love Rust, checkout our [blog](https://www.rerun.io/blog/why-rust), where we talk about some of the reasons.

We depend on a number of third party libraries, most notably:
* [Apache Arrow](https://arrow.apache.org/) for our data store
* [wgpu](https://wgpu.rs/) for rendering
* [egui](https://github.com/emilk/egui) for UI
* [PyO3](https://github.com/PyO3/pyo3) for Python bindings

If you want to learn more about the different parts of the SDK & Viewer and how they work, check out
[this architecture overview](https://github.com/rerun-io/rerun/blob/latest/ARCHITECTURE.md)
for an introduction.
