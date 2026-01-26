Example:

Start a server:
```sh
pixi run rerun-release --serve-grpc --server-memory-limit 1GB
```

Run the benchmark:
```sh
cargo run --release -p log_benchmark -- --profile --connect transforms3d
```
