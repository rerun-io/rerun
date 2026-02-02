//! Intended to be used as an xtask in order to make it trivial to run web-based examples.
//!
//! This is a temporary solution while we're in the process of building our own xtask tools.

use std::time::Duration;

fn main() {
    const CSS: &str = r#"
        html {
            /* Remove touch delay: */
            touch-action: manipulation;
        }

        body {
            /* Light mode background color for what is not covered by the egui canvas,
            or where the egui canvas is translucent. */
            background: #909090;
        }

        @media (prefers-color-scheme: dark) {
            body {
                /* Dark mode background color for what is not covered by the egui canvas,
                or where the egui canvas is translucent. */
                background: #404040;
            }
        }

        /* Allow canvas to fill entire web page: */
        html,
        body {
            overflow: hidden;
            margin: 0 !important;
            padding: 0 !important;
            height: 100%;
            width: 100%;
        }

        /* ---------------------------------------------- */
        /* Loading animation from https://loading.io/css/ */
        .lds-dual-ring {
            display: inline-block;
            width: 24px;
            height: 24px;
        }

        .lds-dual-ring:after {
            content: " ";
            display: block;
            width: 24px;
            height: 24px;
            margin: 0px;
            border-radius: 50%;
            border: 3px solid #fff;
            border-color: #fff transparent #fff transparent;
            animation: lds-dual-ring 1.2s linear infinite;
        }

        @keyframes lds-dual-ring {
            0% {
                transform: rotate(0deg);
            }

            100% {
                transform: rotate(360deg);
            }
        }
    "#;

    use pico_args::Arguments;
    let mut args = Arguments::from_env();
    let host = args
        .opt_value_from_str("--host")
        .unwrap_or(None)
        .unwrap_or_else(|| "localhost".to_owned());
    let port = args
        .opt_value_from_str("--port")
        .unwrap_or(None)
        .unwrap_or_else(|| "8000".to_owned());

    let thread = std::thread::Builder::new()
        .name("cargo_run_wasm".into())
        .spawn(|| {
            cargo_run_wasm::run_wasm_cli_with_css(CSS);
        })
        .expect("Failed to spawn thread");

    if args.contains("--build-only") {
        thread.join().expect("std::thread::join() failed");
    } else {
        // It would be nice to start a web-browser, but we can't really know when the server is ready.
        // So we just sleep for a while and hope it works.
        std::thread::sleep(Duration::from_millis(500));

        // Open browser tab.
        let viewer_url = format!("http://{host}:{port}",);
        webbrowser::open(&viewer_url).ok();
        println!("Opening browser at {viewer_url}");

        std::thread::sleep(Duration::from_secs(u64::MAX));
    }
}
