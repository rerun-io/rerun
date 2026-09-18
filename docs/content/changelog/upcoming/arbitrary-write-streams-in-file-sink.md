---
title: Changeset entry (template)
hidden: true
type: feature
---

### Support creating a FileSink from arbitrary std::io::Write streams

It is now possible to wrap any Rust `std::io::Write` stream in a `FileSink`, using `FileSink::new_stream()` or `FileSink::new_stream_with_options()`.
