---
title: "Mat3x3"
---

A 3x3 Matrix.

Matrices in Rerun are stored as flat list of coefficients in column-major order:
```text
            column 0       column 1       column 2
       -------------------------------------------------
row 0 | flat_columns[0] flat_columns[3] flat_columns[6]
row 1 | flat_columns[1] flat_columns[4] flat_columns[7]
row 2 | flat_columns[2] flat_columns[5] flat_columns[8]
```


## Links
 * 🐍 [Python API docs for `Mat3x3`](https://ref.rerun.io/docs/python/stable/common/datatypes#rerun.datatypes.Mat3x3)
 * 🦀 [Rust API docs for `Mat3x3`](https://docs.rs/rerun/latest/rerun/datatypes/struct.Mat3x3.html)


## Used by

* [`PinholeProjection`](../components/pinhole_projection.md)
* [`TranslationAndMat3x3`](../datatypes/translation_and_mat3x3.md)
