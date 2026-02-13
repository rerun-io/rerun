//! Used to track viewer history. This is used when web history isn't used, i.e for
//! the native viewer and for web viewers that can't control the history.
//!
//! This, much like web history, tracks viewer urls. And pushes new history whenever
//! the non-fragment part of an url changes.

use re_viewer_context::open_url::ViewerOpenUrl;

/// History of url's the viewer has visited.
#[derive(Default)]
pub struct History {
    entries: Vec<ViewerOpenUrl>,
    current_entry: usize,
}

impl History {
    pub fn update_current_url(&mut self, url: ViewerOpenUrl) {
        // If just the fragment changed we don't push a new entry.
        if let Some(current) = self.entries.get_mut(self.current_entry)
            && current.clone().without_fragment() == url.clone().without_fragment()
        {
            *current = url;
        } else {
            // Clear forward history when a new entry is pushed.
            if self.current_entry + 1 < self.entries.len() {
                self.entries.drain(self.current_entry + 1..);
            }

            self.entries.push(url);
            if self.current_entry + 1 < self.entries.len() {
                self.current_entry += 1;
            }
        }
    }

    /// Goes back in history, returning the new url to open.
    pub fn go_back(&mut self) -> Option<&ViewerOpenUrl> {
        self.current_entry = self.current_entry.checked_sub(1)?;

        self.entries.get(self.current_entry)
    }

    /// Goes forward in history, returning the new url to open.
    pub fn go_forward(&mut self) -> Option<&ViewerOpenUrl> {
        let new_entry = self.current_entry + 1;

        if new_entry >= self.entries.len() {
            return None;
        }

        self.current_entry = new_entry;

        self.entries.get(self.current_entry)
    }

    /// Is there history to go back to?
    pub fn has_back(&self) -> bool {
        self.current_entry > 0
    }

    /// Is there history to go forward to?
    pub fn has_forward(&self) -> bool {
        self.current_entry + 1 < self.entries.len()
    }
}
