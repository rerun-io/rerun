fn main() {
    // https://blog.rust-lang.org/2024/05/06/check-cfg.html
    // See `Cargo.toml` docs for info about `__disable_server` and `RERUN_DISABLE_WEB_VIEWER_SERVER`.
    println!("cargo::rustc-check-cfg=cfg(disable_web_viewer_server)");

    let disable_web_viewer_server =
        re_build_tools::is_tracked_env_var_set("RERUN_DISABLE_WEB_VIEWER_SERVER")
            || cfg!(feature = "__disable_server");

    if disable_web_viewer_server {
        println!("cargo::rustc-cfg=disable_web_viewer_server");
    }

    let needs_wasm = !disable_web_viewer_server;
    if needs_wasm {
        let viewer_js_path = std::path::Path::new("./web_viewer/re_viewer.js");
        let viewer_wasm_path = std::path::Path::new("./web_viewer/re_viewer_bg.wasm");

        if !viewer_js_path.exists() || !viewer_wasm_path.exists() {
            // Detect build profile to choose the right pixi command
            let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
            let is_release = profile == "release" || profile == "web-release";

            let pixi_command = if is_release {
                "rerun-build-web-release"
            } else {
                "rerun-build-web"
            };

            eprintln!("Web viewer not found, building it automatically...");
            eprintln!("Running: pixi run {}\n", pixi_command);

            // Try to automatically build the web viewer
            let mut command = std::process::Command::new("pixi");
            command.args(["run", pixi_command]);

            // Running `cargo` from within a build-script inherits the workspace lock which
            // causes nested invocations to wait forever.  `pixi run` ultimately launches a
            // `cargo run` which needs its own target directory in order to avoid that lock.
            // Compute the workspace target directory from `OUT_DIR` and point the nested
            // invocation at a separate subdirectory.
            if let Ok(out_dir) = std::env::var("OUT_DIR") {
                let target_dir = std::path::Path::new(&out_dir)
                    .ancestors()
                    .find(|path| path.file_name().map_or(false, |name| name == "target"))
                    .or_else(|| std::path::Path::new(&out_dir).ancestors().nth(3));

                if let Some(target_dir) = target_dir {
                    command.env(
                        "CARGO_TARGET_DIR",
                        target_dir.join("web_viewer_build"),
                    );
                }
            }

            let result = command.status();

            match result {
                Ok(status) if status.success() => {
                    eprintln!("\n✓ Web viewer built successfully!\n");
                    // Verify the files were actually created
                    if !viewer_js_path.exists() || !viewer_wasm_path.exists() {
                        panic!(
                            "\n\nWeb viewer build succeeded but files are still missing!\n\
                             Expected files:\n  • {}\n  • {}\n",
                            viewer_js_path.display(),
                            viewer_wasm_path.display()
                        );
                    }
                }
                Ok(status) => {
                    // Pixi command failed, show helpful error
                    panic!(
                        "\n\n\
                        ╔══════════════════════════════════════════════════════════════════╗\n\
                        ║  Failed to build web viewer automatically                        ║\n\
                        ╠══════════════════════════════════════════════════════════════════╣\n\
                        ║  Command 'pixi run {}' failed with exit code: {:?}         ║\n\
                        ║                                                                  ║\n\
                        ║  Please try building manually:                                   ║\n\
                        ║    Debug:   pixi run rerun-build-web                             ║\n\
                        ║    Release: pixi run rerun-build-web-release                     ║\n\
                        ║                                                                  ║\n\
                        ║  Or disable the web viewer server:                               ║\n\
                        ║    export RERUN_DISABLE_WEB_VIEWER_SERVER=1                      ║\n\
                        ╚══════════════════════════════════════════════════════════════════╝\n",
                        pixi_command,
                        status.code()
                    );
                }
                Err(e) => {
                    // Pixi not found or other error
                    panic!(
                        "\n\n\
                        ╔══════════════════════════════════════════════════════════════════╗\n\
                        ║  Failed to execute pixi command                                  ║\n\
                        ╠══════════════════════════════════════════════════════════════════╣\n\
                        ║  Error: {:<59} ║\n\
                        ║                                                                  ║\n\
                        ║  Make sure 'pixi' is installed and in your PATH                 ║\n\
                        ║                                                                  ║\n\
                        ║  Or build the web viewer manually:                               ║\n\
                        ║    pixi run rerun-build-web                                      ║\n\
                        ║                                                                  ║\n\
                        ║  Or disable the web viewer server:                               ║\n\
                        ║    export RERUN_DISABLE_WEB_VIEWER_SERVER=1                      ║\n\
                        ╚══════════════════════════════════════════════════════════════════╝\n",
                        e
                    );
                }
            }
        }
    }
}
