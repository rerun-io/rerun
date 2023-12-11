Standalone examples for `re_renderer`
-----------------------------------------------

To get a list of all examples run:
```
cargo run -p re_renderer_examples --bin
```

E.g. to run the `multiview`` example run 
```
cargo run -p re_renderer_examples --bin multiview
```
To run the same example on the web using WebGPU:
```
cargo run-wasm -p re_renderer_examples --bin multiview
```
To run the same example on the web using WebGL:
```
cargo run-wasm -p re_renderer_examples --bin multiview --features "webgl"
```
