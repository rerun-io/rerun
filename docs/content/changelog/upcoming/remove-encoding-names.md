---
title: Removed deprecated SDK datatype namespaces
hidden: true
type: breaking
---

Rerun 0.44 removes the `datatypes` namespaces deprecated in 0.37.

Use `encodings` instead of `datatypes` in Rust and Python, including blueprint imports.
In C++, include `<rerun/encodings.hpp>` and use `rerun::encodings` instead of `<rerun/datatypes.hpp>` and `rerun::datatypes`.
