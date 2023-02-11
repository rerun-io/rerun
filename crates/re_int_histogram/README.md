# re_int_histogram

A histogram with `i64` keys and `u32` counts, supporting both sparse and dense uses.

It supports high-level summaries of the histogram, so that you can quickly get a birds-eye view of the data without having to visit every point in the histogram.

You can also think of the histogram as a multi-set, where you can insert the same key multiple times and then query how many times you've inserted it.

Used for noting at which times we have events, so that we can visualize it in the time panel.
