#![cfg(feature = "setup")]

fn emit_deduplicated_warning(value: &str) {
    re_log::warn_once!("Deduplicated warning: {value}");
}

#[test]
fn log_once_deduplicates_messages() {
    let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::INFO);
    re_log::setup_logging();

    emit_deduplicated_warning("first");
    emit_deduplicated_warning("first");
    emit_deduplicated_warning("second");

    let messages = log_rx
        .try_iter()
        .filter(|log_msg| log_msg.message.starts_with("Deduplicated warning:"))
        .map(|log_msg| log_msg.message)
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "Deduplicated warning: first",
            "Deduplicated warning: second"
        ]
    );
}

#[test]
fn channel_logger_preserves_targets() {
    let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::INFO);
    re_log::setup_logging();

    re_log::warn!(target: "re_log_test", "Regular targeted warning");
    re_log::warn_once!("Once warning");
    re_log::external::log::warn!(target: "re_log_test", "Facade targeted warning");

    let log_messages = log_rx.try_iter().collect::<Vec<_>>();
    for (message, expected_target) in [
        ("Regular targeted warning", "re_log_test"),
        ("Once warning", "channel_logger"),
        ("Facade targeted warning", "re_log_test"),
    ] {
        let log_msg = log_messages
            .iter()
            .find(|log_msg| log_msg.message == message)
            .expect("the channel logger should capture the warning");
        assert_eq!(log_msg.target, expected_target);
        assert!(log_msg.fields.is_empty());
    }
}
