//! Progressive feature discovery: at most one attention dot is visible at a time.
//!
//! New users should not be greeted by a wall of dots.
//! Instead, each [`Feature`] becomes eligible for a dot only after the viewer
//! has been launched a few times, and only the highest-priority eligible feature
//! gets a dot. An unread notification suppresses all discovery dots.
//!
//! Candidates register during a frame, and the winner is picked at the end of it,
//! so the order in which features are drawn does not affect which one wins.
//!
//! One-shot [`Message`]s, such as nags shown at startup, are gated on the
//! launch count in the same way, but take no part in the dot arbitration.

/// A feature that we want users to discover, in priority order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    /// The agent panel toggle in the top panel.
    AgentPanel,
}

/// A one-shot message that should not bother first-time users.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    /// Check for a newer Rerun version, and tell the user if there is one.
    VersionCheck,

    /// Suggest the native viewer to users of the web viewer.
    NativeViewerTip,
}

/// Previous launch count to assume for users who ran Rerun before launches were counted.
///
/// They are not new, so this unlocks everything.
pub const PREVIOUS_LAUNCHES_OF_RETURNING_USER: u32 = 3;

// The launch schedule. Launches are counted from 1, and the first launch is always quiet.
// Each later launch introduces at most one new thing.

impl Feature {
    /// The first launch on which this feature may show a dot.
    fn first_launch(self) -> u32 {
        match self {
            Self::AgentPanel => 2,
        }
    }
}

impl Message {
    /// The first launch on which this message may be shown.
    fn first_launch(self) -> u32 {
        match self {
            Self::VersionCheck => 3,
            Self::NativeViewerTip => 4,
        }
    }

    /// Is it time to show this message, given how many times the viewer has been launched?
    pub fn is_unlocked(self, ctx: &egui::Context) -> bool {
        with_state(ctx, |state| self.first_launch() <= state.launch_count).unwrap_or(false)
    }
}

/// Lives as an [`egui::Plugin`], so it survives resets of egui memory.
#[derive(Clone, Debug, Default)]
struct DiscoveryState {
    launch_count: u32,

    /// Best candidate registered during the current pass.
    candidate: Option<Feature>,
    notification_dot_this_pass: bool,

    /// The feature that gets a dot this pass, decided at the end of the previous pass.
    winner: Option<Feature>,

    /// Set once the winner has painted its dot this pass, so it only shows once.
    winner_painted: bool,
}

impl DiscoveryState {
    fn claim_dot(&mut self, feature: Feature, already_discovered: bool) -> bool {
        if already_discovered || self.launch_count < feature.first_launch() {
            return false;
        }

        if self.candidate.is_none_or(|candidate| feature < candidate) {
            self.candidate = Some(feature);
        }

        if self.winner == Some(feature) && !self.winner_painted {
            self.winner_painted = true;
            true
        } else {
            false
        }
    }

    /// Picks the winner for the next pass.
    ///
    /// Returns `true` if the winner changed, which needs a repaint to show.
    fn end_pass(&mut self) -> bool {
        let previous_winner = self.winner;
        self.winner = if self.notification_dot_this_pass {
            None
        } else {
            self.candidate
        };
        self.candidate = None;
        self.notification_dot_this_pass = false;
        self.winner_painted = false;
        self.winner != previous_winner
    }
}

impl egui::Plugin for DiscoveryState {
    fn debug_name(&self) -> &'static str {
        "re_ui::discovery"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        if self.end_pass() {
            ui.ctx().request_repaint();
        }
    }
}

/// Returns `None` if [`install`] was never called.
fn with_state<R>(ctx: &egui::Context, f: impl FnOnce(&mut DiscoveryState) -> R) -> Option<R> {
    ctx.with_plugin(f)
}

/// Enable discovery dots and messages for this context.
///
/// `launch_count` is how many times the viewer has been launched, counting
/// the current launch. Zero disables everything.
pub fn install(ctx: &egui::Context, launch_count: u32) {
    ctx.add_plugin(DiscoveryState {
        launch_count,
        ..Default::default()
    });
}

/// Change the launch count after [`install`], e.g. when the user resets the viewer.
pub fn set_launch_count(ctx: &egui::Context, launch_count: u32) {
    with_state(ctx, |state| state.launch_count = launch_count);
}

/// Report that the notification bell shows an unread dot this frame.
///
/// This hides all discovery dots, so that the notification stands out.
pub fn report_notification_dot(ctx: &egui::Context) {
    with_state(ctx, |state| state.notification_dot_this_pass = true);
}

/// Registers `feature` as wanting a dot, and returns `true` if the caller should paint it.
///
/// `already_discovered` is whether the user has already used the feature.
/// Returns `true` at most once per frame, so if several buttons share a
/// feature, only the first one drawn gets the dot.
pub fn claim_dot(ctx: &egui::Context, feature: Feature, already_discovered: bool) -> bool {
    with_state(ctx, |state| state.claim_dot(feature, already_discovered)).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs one pass, calling `claim_dot` for each `(feature, already_discovered)`
    /// in order, and returns which calls got a dot.
    fn pass(
        state: &mut DiscoveryState,
        notification: bool,
        calls: &[(Feature, bool)],
    ) -> Vec<bool> {
        state.notification_dot_this_pass = notification;
        let result = calls
            .iter()
            .map(|&(feature, already_discovered)| state.claim_dot(feature, already_discovered))
            .collect();
        state.end_pass();
        result
    }

    fn state_with_launch_count(launch_count: u32) -> DiscoveryState {
        DiscoveryState {
            launch_count,
            ..Default::default()
        }
    }

    #[test]
    fn returning_users_have_everything_unlocked() {
        let launch_count = PREVIOUS_LAUNCHES_OF_RETURNING_USER + 1;
        assert!(Feature::AgentPanel.first_launch() <= launch_count);
        assert!(Message::VersionCheck.first_launch() <= launch_count);
        assert!(Message::NativeViewerTip.first_launch() <= launch_count);
    }

    #[test]
    fn at_most_one_new_thing_per_launch() {
        let mut first_launches = vec![
            Feature::AgentPanel.first_launch(),
            Message::VersionCheck.first_launch(),
            Message::NativeViewerTip.first_launch(),
        ];
        let count = first_launches.len();
        first_launches.sort_unstable();
        first_launches.dedup();
        assert_eq!(first_launches.len(), count);
    }

    #[test]
    fn first_launch_is_quiet() {
        assert!(1 < Feature::AgentPanel.first_launch());
        assert!(1 < Message::VersionCheck.first_launch());
        assert!(1 < Message::NativeViewerTip.first_launch());
    }

    #[test]
    fn no_dots_without_launch_count_or_on_first_launch() {
        for launch_count in [0, 1] {
            let mut state = state_with_launch_count(launch_count);
            let calls = [(Feature::AgentPanel, false)];
            pass(&mut state, false, &calls);
            assert_eq!(pass(&mut state, false, &calls), [false]);
        }
    }

    #[test]
    fn undiscovered_feature_gets_a_dot_from_next_pass() {
        let mut state = state_with_launch_count(2);
        let calls = [(Feature::AgentPanel, false)];
        assert_eq!(
            pass(&mut state, false, &calls),
            [false],
            "decided at end of pass"
        );
        assert_eq!(pass(&mut state, false, &calls), [true]);
        assert_eq!(
            pass(&mut state, false, &[(Feature::AgentPanel, true)]),
            [false]
        );
        assert_eq!(
            pass(&mut state, false, &calls),
            [false],
            "discovered last pass"
        );
    }

    #[test]
    fn shared_feature_dot_is_painted_once() {
        let mut state = state_with_launch_count(2);
        let calls = [(Feature::AgentPanel, false), (Feature::AgentPanel, false)];
        pass(&mut state, false, &calls);
        assert_eq!(pass(&mut state, false, &calls), [true, false]);
    }

    #[test]
    fn notification_hides_discovery_dots() {
        let mut state = state_with_launch_count(2);
        let calls = [(Feature::AgentPanel, false)];
        pass(&mut state, true, &calls);
        assert_eq!(pass(&mut state, true, &calls), [false]);
        assert_eq!(
            pass(&mut state, false, &calls),
            [false],
            "decided at end of pass"
        );
        assert_eq!(pass(&mut state, false, &calls), [true]);
    }

    #[test]
    fn end_pass_reports_winner_changes() {
        let mut state = state_with_launch_count(2);
        state.claim_dot(Feature::AgentPanel, false);
        assert!(state.end_pass(), "new winner");
        state.claim_dot(Feature::AgentPanel, false);
        assert!(!state.end_pass(), "same winner");
        state.claim_dot(Feature::AgentPanel, true);
        assert!(state.end_pass(), "winner discovered");
    }
}
