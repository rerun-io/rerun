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


To run the same example on the web:
```
cargo run-wasm -p re_renderer_examples --bin multiview
```
Note that this will try to use WebGPU and fall back to WebGL if WebGPU is not supported by your browser.
