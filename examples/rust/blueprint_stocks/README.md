This example fetches the last 5 days of stock data for a few different stocks.
We show how Rerun blueprints can then be used to present many different views of the same data.

```bash
# default grid layout
cargo run -p blueprint_stocks

# blueprint modes
cargo run -p blueprint_stocks -- --blueprint auto
cargo run -p blueprint_stocks -- --blueprint one-stock
cargo run -p blueprint_stocks -- --blueprint one-stock-with-info
cargo run -p blueprint_stocks -- --blueprint compare-two
cargo run -p blueprint_stocks -- --blueprint one-stock-no-peaks
cargo run -p blueprint_stocks -- --blueprint grid

# show time + selection panels
cargo run -p blueprint_stocks -- --show_panels
```
