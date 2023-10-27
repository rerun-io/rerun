//! Shows integration of Rerun's `TextLog` with the native logging interface.

use rerun::external::log;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_log_integration").spawn()?;

    // Log a text entry directly:
    rec.log(
        "logs",
        &rerun::TextLog::new("this entry has loglevel TRACE")
            .with_level(rerun::TextLogLevel::TRACE),
    )?;

    // Or log via a logging handler:
    rerun::Logger::new(rec.clone()) // recording streams are ref-counted
        .with_path_prefix("logs/handler")
        // You can also use the standard `RUST_LOG` environment variable!
        .with_filter(rerun::default_log_filter())
        .init()?;
    log::info!("This INFO log got added through the standard logging interface");

    log::logger().flush();

    Ok(())
}
