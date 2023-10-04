---
title: "Mat4x4"
---

A 4x4 Matrix.

Matrices in Rerun are stored as flat list of coefficients in column-major order:
```text
           column 0         column 1         column 2         column 3
       --------------------------------------------------------------------
row 0 | flat_columns[0]  flat_columns[4]  flat_columns[8]  flat_columns[12]
row 1 | flat_columns[1]  flat_columns[5]  flat_columns[9]  flat_columns[13]
row 2 | flat_columns[2]  flat_columns[6]  flat_columns[10] flat_columns[14]
row 3 | flat_columns[3]  flat_columns[7]  flat_columns[11] flat_columns[15]
```


## Links
 * 🐍 Python API docs: https://ref.rerun.io/docs/python/HEAD/package/rerun/datatypes/mat4x4/
 * 🦀 Rust API docs: https://docs.rs/rerun/latest/rerun/datatypes/struct.Mat4x4.html


