# re_int_histogram
Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.


[![Latest version](https://img.shields.io/crates/v/re_int_histogram.svg)](https://crates.io/crates/re_int_histogram)
[![Documentation](https://docs.rs/re_int_histogram/badge.svg)](https://docs.rs/re_int_histogram)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A histogram with `i64` keys and `u32` counts, supporting both sparse and dense uses.

It supports high-level summaries of the histogram, so that you can quickly get a birds-eye view of the data without having to visit every point in the histogram.

You can also think of the histogram as a multi-set, where you can insert the same key multiple times and then query how many times you've inserted it.

Used for noting at which times we have events, so that we can visualize it in the time panel.
