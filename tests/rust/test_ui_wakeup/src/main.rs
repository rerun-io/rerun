//! Test that the Rerun Viewer UI wakes up as new messages arrive,
//! even if the viewer is hidden.
//!
//! ## Test setup - build the viewer
//! * `pixi run rerun-build`
//! * `pixi run rerun-build-web`
//!
//! ## Test matrix
//! * Run `cargo r -p test_ui_wakeup` and test:
//!   * That the viewer wakes up in the background when it's alt-tabbed
//!   * That the viewer wakes up when minimized (it should log "Received a message from…")
//! * Run `cargo r -p test_ui_wakeup -- --serve` and test:
//!   * The viewer wakes up when browser is alt-tabbed away
//!   * Switch to a different browser tab, send a few messages, switch back. The messages should be there
//!     (this is not a conclusive test, as the messages might have been received on tab select)
//! * Run `cargo r -p test_ui_wakeup -- --save stream.rrd` and in another terminal start the viewer with `pixi run rerun stream.rrd` and test:
//!  * The viewer is updated on every new message (every ENTER press)

use std::io::Read as _;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    // This is so that re_viewer logs incoming messages:
    let rust_log = "info,re_viewer=trace";
    eprintln!("Setting RUST_LOG={rust_log}");

    #[expect(unsafe_code)] // OK: No multithreading here
    unsafe {
        std::env::set_var("RUST_LOG", rust_log)
    };

    println!("Starting Viewer…");
    let (rec, _serve_guard) = args.rerun.init("rerun_example_ui_wakeup")?;

    // Wait out some log spam from the viewer starting:
    std::thread::sleep(std::time::Duration::from_secs(1));

    println!("Now put the viewer in the background (alt-tab, minimize, put in background tab, etc");

    for i in 0.. {
        println!("Sending message number {i}…");
        rec.log(
            "Text",
            &rerun::TextDocument::new(format!("This is message number {i}")),
        )?;
        println!("Press ENTER to send more data to the viewer");

        wait_from_enter();
    }

    Ok(())
}

fn wait_from_enter() {
    let _ = std::io::stdin()
        .read(&mut [0u8])
        .expect("Failed to read from stdin");
}
