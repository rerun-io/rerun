---
title: External data-loader example
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/external_data_loader/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/external_data_loader/src/main.rs
cpp: https://github.com/rerun-io/rerun/tree/latest/examples/cpp/external_data_loader/main.cpp
thumbnail: https://static.rerun.io/external_data_loader_py/6c5609f5dd7d1de373c81babe19221b72d616da3/480w.png
thumbnail_dimensions: [480, 302]
---

<picture>
  <img src="https://static.rerun.io/external_data_loader_py/6c5609f5dd7d1de373c81babe19221b72d616da3/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/external_data_loader_py/6c5609f5dd7d1de373c81babe19221b72d616da3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/external_data_loader_py/6c5609f5dd7d1de373c81babe19221b72d616da3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/external_data_loader_py/6c5609f5dd7d1de373c81babe19221b72d616da3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/external_data_loader_py/6c5609f5dd7d1de373c81babe19221b72d616da3/1200w.png">
</picture>

This is an example executable data-loader plugin for the Rerun Viewer.

It will log Python source code files as markdown documents.

On Linux & Mac you can simply copy it in your $PATH as `rerun-loader-python-file`, then open a Python source file with Rerun (`rerun file.py`).

On Windows you have to install the script as an executable first and then put the executable under %PATH%.
One way to do this is to use `pyinstaller`: `pyinstaller .\examples\python\external_data_loader\main.py -n rerun-loader-python-file --onefile`
