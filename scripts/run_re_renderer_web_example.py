#!/usr/bin/env python3
"""Build a `re_renderer_examples` binary as Wasm and serve it on localhost, so it can be run in a browser."""

from __future__ import annotations

import argparse
import functools
import http.server
import os
import webbrowser

from build_web_viewer import check_wasm_bindgen_cli, run, wasm_target_dir

PACKAGE = "re_renderer_examples"

INDEX_HTML = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{name}</title>
<style>
html, body {{ margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; }}
body {{ background: #909090; touch-action: manipulation; }}
@media (prefers-color-scheme: dark) {{ body {{ background: #404040; }} }}
</style>
</head>
<body>
<script type="module">
import init from "./{name}.js";
window.addEventListener("load", () => init());
</script>
</body>
</html>
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("example", help="Name of the `re_renderer_examples` binary, e.g. `multiview`.")
    parser.add_argument("--release", action="store_true", help="Build with the `release` profile.")
    parser.add_argument("--port", type=int, default=8000, help="Port to serve on (default: 8000).")
    parser.add_argument("--build-only", action="store_true", help="Build, but do not serve.")
    args = parser.parse_args()

    check_wasm_bindgen_cli()

    target_dir = wasm_target_dir()
    cmd = [
        "cargo",
        "build",
        f"--package={PACKAGE}",
        f"--bin={args.example}",
        "--target=wasm32-unknown-unknown",
        f"--target-dir={target_dir}",
        "--config=.cargo/config.toml",
    ]
    if args.release:
        cmd.append("--release")
    # Cargo ignores the wasm32 rustflags from `.cargo/config.toml` when `RUSTFLAGS` is set.
    env = {k: v for k, v in os.environ.items() if k not in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    run(cmd, env=env)

    profile = "release" if args.release else "debug"
    wasm_path = target_dir / "wasm32-unknown-unknown" / profile / f"{args.example}.wasm"
    out_dir = target_dir / PACKAGE / args.example
    out_dir.mkdir(parents=True, exist_ok=True)
    run([
        "wasm-bindgen",
        str(wasm_path),
        f"--out-dir={out_dir}",
        f"--out-name={args.example}",
        "--target=web",
        "--no-typescript",
    ])
    (out_dir / "index.html").write_text(INDEX_HTML.format(name=args.example), encoding="utf-8")
    print(f"Output: {out_dir}", flush=True)

    if args.build_only:
        return

    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(out_dir))
    url = f"http://localhost:{args.port}"
    with http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler) as server:
        print(f"Serving at {url} (Ctrl-C to stop)", flush=True)
        webbrowser.open(url)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
