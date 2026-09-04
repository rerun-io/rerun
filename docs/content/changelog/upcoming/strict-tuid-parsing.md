---
title: "TUID parsing requires canonical 32-character hexadecimal strings"
hidden: true
type: breaking
---

### TUID parsing requires canonical 32-character hexadecimal strings

`Tuid::from_str` now accepts only exactly 32 ASCII hexadecimal characters (`0-9`, `a-f`, and `A-F`).
Short inputs and inputs with a leading sign, which previously parsed as `u128` values, now return `ParseTuidError`.
The canonical 32-character form produced by `Tuid::Display` continues to roundtrip.

Use a zero-padded canonical string when constructing a `Tuid` from text:

```rust
let tuid: Tuid = "1".parse()?; // before
let tuid: Tuid = "00000000000000000000000000000001".parse()?; // after
```
