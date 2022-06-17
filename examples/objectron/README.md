```sh
./download_dataset.py
cargo run --release -p objectron -- objectron/dataset/camera/batch-5/31
```

To test the web viewer:
```
./crates/re_viewer/build_web.sh
cargo run --release -p objectron --features web -- objectron/dataset/camera/batch-5/31 --web --open
```
