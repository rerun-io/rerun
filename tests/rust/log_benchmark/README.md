
## Profile log performance
Start a server:
```sh
pixi run rerun-release --serve-grpc --server-memory-limit 1GB
```

Run the logging:
```sh
cargo run --release -p log_benchmark -- --connect --profile scalars --num-entities 10 --num-time-steps 100000
```

## Profile ingestion performance
Start a server:
```sh
pixi run rerun-release --serve-grpc --server-memory-limit 1GB
```

Start the viewer with a profiler attached:
```sh
pixi run rerun-release --profile --connect
```

Start logging:
```sh
cargo run --release -p log_benchmark -- --connect scalars --num-entities 10 --num-time-steps 100000
```
