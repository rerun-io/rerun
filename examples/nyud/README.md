Using https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html

Setup:
``` sh
(cd dataset && wget http://horatio.cs.nyu.edu/mit/silberman/nyu_depth_v2/cafe.zip)
```

Run:
``` sh
cargo run --release -p nyud -- examples/nyud/dataset/cafe.zip
```
