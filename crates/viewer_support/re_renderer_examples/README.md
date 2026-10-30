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
pixi run re-renderer-web-example multiview
```
Note that this will try to use WebGPU and fall back to WebGL if WebGPU is not supported by your browser.
