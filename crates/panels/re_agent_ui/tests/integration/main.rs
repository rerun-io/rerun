mod agent_panel_snapshots;
mod chat_input;
mod copy_response;
mod host_button;
mod new_conversation_shortcut;
mod pasted_images;

/// Shows hover timestamps in UTC, so snapshots do not depend on where they run.
///
/// Call it first in the test: setting an env var is only sound while no other thread is running.
/// Nextest runs each test in its own process, so this affects no other test.
fn use_utc() {
    #[expect(unsafe_code)]
    // SAFETY: called before the test starts any other thread.
    unsafe {
        std::env::set_var("TZ", "UTC");
    }
}

/// Gives every transcript entry the same fixed creation time, on another day than today, so
/// hover timestamps in snapshots do not depend on when the test runs.
fn pin_entry_times(session: &mut re_agent_ui::AgentSession) {
    let created_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    for entry in &mut session.transcript_mut().items {
        entry.created_at = created_at;
    }
}
