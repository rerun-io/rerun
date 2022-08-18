```sh
(cd examples/objectron && ./download_dataset.py)
cargo run --release -p objectron -- examples/objectron/dataset/camera/batch-5/31
```

To test the web viewer:
```
./crates/re_viewer/build_web.sh
cargo run --release -p objectron --features web -- examples/objectron/dataset/camera/batch-5/31 --web --open
```
