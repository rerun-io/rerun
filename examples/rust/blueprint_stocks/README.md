This example logs stock data to show how Rerun blueprints can be used to present different views of the same data. Very similar to Python's `blueprint_stocks` example, but using static (not live) data.

```bash
# default grid layout
cargo run -p blueprint_stocks

# blueprint modes
cargo run -p blueprint_stocks -- --blueprint auto
cargo run -p blueprint_stocks -- --blueprint one-stock
cargo run -p blueprint_stocks -- --blueprint one-stock-no-peaks
cargo run -p blueprint_stocks -- --blueprint one-stock-with-info
cargo run -p blueprint_stocks -- --blueprint compare-two
cargo run -p blueprint_stocks -- --blueprint grid

# show time + selection panels
cargo run -p blueprint_stocks -- --show_panels
```
