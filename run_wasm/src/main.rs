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
    cargo_run_wasm::run_wasm_with_css(CSS);
}
