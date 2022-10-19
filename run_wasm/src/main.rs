fn main() {
    // TODO(cmc): Why is this not taking the full screen?
    const CSS: &'static str = r#"
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

        /* Position canvas in center-top: */
        canvas {
            margin-right: auto;
            margin-left: auto;
            display: block;
            position: absolute;
            top: 0%;
            left: 50%;
            transform: translate(-50%, 0%);
        }

        .centered {
            margin-right: auto;
            margin-left: auto;
            display: block;
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            color: #f0f0f0;
            font-size: 24px;
            font-family: Ubuntu-Light, Helvetica, sans-serif;
            text-align: center;
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

    std::thread::spawn(|| {
        // Just a convenience hack so that the tab is likely to open after the
        // server is fully booted.
        // Will do this the right way once we get our own xtask thing.
        std::thread::sleep(std::time::Duration::from_millis(500));

        use pico_args::Arguments;
        let mut args = Arguments::from_env();
        let host: Option<String> = args.opt_value_from_str("--host").unwrap();
        let port: Option<String> = args.opt_value_from_str("--port").unwrap();

        let viewer_url = format!(
            "http://{}:{}",
            host.as_deref().unwrap_or("localhost"),
            port.as_deref().unwrap_or("8000")
        );
        webbrowser::open(&viewer_url).ok();

        println!("Opening browser at {viewer_url}");
    });

    cargo_run_wasm::run_wasm_with_css(CSS);
}
