<!--[metadata]
title = "Rust blueprint example"
thumbnail = "https://static.rerun.io/rust_blueprint/b78d5645741c57fec26c9214e051de571fd70771/480w.png"
thumbnail_dimensions = [480, 285]
-->
Example of using the blueprint APIs to configure Rerun.

## Running the example

```bash
# default blueprint
cargo run -p blueprint
# Don't send the blueprint
cargo run -p blueprint -- --skip-blueprint
# Automatically add views
cargo run -p blueprint -- --auto-views
```
