//! The viewer's recent log messages, kept so that `re_viewer_mcp` can hand them to an agent.

use std::collections::VecDeque;

use re_protos::sdk_comms::v1alpha1::ViewerLogEntry;

/// Older entries are dropped once the buffer holds this many.
const MAX_ENTRIES: usize = 1000;

/// A bounded buffer of log messages, each numbered so that clients can fetch incrementally.
#[derive(Default)]
pub struct ViewerLog {
    entries: VecDeque<ViewerLogEntry>,
    next_sequence: u64,
}

impl ViewerLog {
    pub fn push(&mut self, log_msg: &re_log::LogMsg) {
        let re_log::LogMsg {
            level,
            target,
            message,
            fields: _,
        } = log_msg;

        self.next_sequence += 1;
        self.entries.push_back(ViewerLogEntry {
            sequence: self.next_sequence,
            level: level.to_string(),
            target: target.clone(),
            message: message.clone(),
        });
        while MAX_ENTRIES < self.entries.len() {
            self.entries.pop_front();
        }
    }

    /// Entries with a sequence number greater than `after_sequence`, oldest first.
    pub fn entries_after(&self, after_sequence: Option<u64>) -> Vec<ViewerLogEntry> {
        let after_sequence = after_sequence.unwrap_or(0);
        self.entries
            .iter()
            .filter(|entry| after_sequence < entry.sequence)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(message: &str) -> re_log::LogMsg {
        re_log::LogMsg {
            level: re_log::Level::WARN,
            target: "test".to_owned(),
            message: message.to_owned(),
            fields: Vec::new(),
        }
    }

    #[test]
    fn sequences_and_cursor() {
        let mut log = ViewerLog::default();
        assert!(log.entries_after(None).is_empty());

        log.push(&msg("a"));
        log.push(&msg("b"));
        log.push(&msg("c"));

        let all = log.entries_after(None);
        assert_eq!(
            all.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(all[0].level, "WARN");

        let after_two = log.entries_after(Some(2));
        assert_eq!(after_two.len(), 1);
        assert_eq!(after_two[0].message, "c");
        assert!(log.entries_after(Some(3)).is_empty());
        assert!(log.entries_after(Some(99)).is_empty());
    }

    #[test]
    fn bounded() {
        let mut log = ViewerLog::default();
        for i in 0..(MAX_ENTRIES + 5) {
            log.push(&msg(&i.to_string()));
        }
        let all = log.entries_after(None);
        assert_eq!(all.len(), MAX_ENTRIES);
        assert_eq!(all[0].sequence, 6);
        assert_eq!(all[0].message, "5");
    }
}
