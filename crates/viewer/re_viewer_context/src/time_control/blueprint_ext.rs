use re_chunk::{EntityPath, TimelineName};
use re_log_types::AbsoluteTimeRange;
use re_sdk_types::blueprint::archetypes::TimePanelBlueprint;
use re_sdk_types::blueprint::components::{LoopMode, PlayState};

use crate::blueprint_helpers::BlueprintContext;

pub const TIME_PANEL_PATH: &str = "time_panel";

pub fn time_panel_blueprint_entity_path() -> EntityPath {
    TIME_PANEL_PATH.into()
}

/// Helper trait to write time panel related blueprint components.
pub(super) trait TimeBlueprintExt {
    fn set_timeline(&self, timeline: TimelineName);

    fn timeline(&self) -> Option<TimelineName>;

    /// Replaces the current timeline with the automatic one.
    fn clear_timeline(&self);

    fn set_playback_speed(&self, playback_speed: f64);
    fn playback_speed(&self) -> Option<f64>;

    fn set_fps(&self, fps: f64);
    fn fps(&self) -> Option<f64>;

    fn set_play_state(&self, play_state: PlayState);
    fn play_state(&self) -> Option<PlayState>;

    fn set_loop_mode(&self, loop_mode: LoopMode);
    fn loop_mode(&self) -> Option<LoopMode>;

    fn set_time_selection(&self, time_range: AbsoluteTimeRange);
    fn time_selection(&self) -> Option<AbsoluteTimeRange>;
    fn clear_time_selection(&self);
}

impl<T: BlueprintContext> TimeBlueprintExt for T {
    fn set_timeline(&self, timeline: TimelineName) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_timeline(),
            &re_sdk_types::blueprint::components::TimelineName::from(timeline.as_str()),
        );
    }

    fn timeline(&self) -> Option<TimelineName> {
        let (_, timeline) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::TimelineName>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_timeline().component,
        )?;

        Some(TimelineName::new(timeline.as_str()))
    }

    fn clear_timeline(&self) {
        self.clear_blueprint_component(
            time_panel_blueprint_entity_path(),
            TimePanelBlueprint::descriptor_timeline(),
        );
    }

    fn set_playback_speed(&self, playback_speed: f64) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_playback_speed(),
            &re_sdk_types::blueprint::components::PlaybackSpeed(playback_speed.into()),
        );
    }

    fn playback_speed(&self) -> Option<f64> {
        let (_, playback_speed) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::PlaybackSpeed>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_playback_speed().component,
        )?;

        Some(**playback_speed)
    }

    fn set_fps(&self, fps: f64) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_fps(),
            &re_sdk_types::blueprint::components::Fps(fps.into()),
        );
    }

    fn fps(&self) -> Option<f64> {
        let (_, fps) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::Fps>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_fps().component,
            )?;

        Some(**fps)
    }

    fn set_play_state(&self, play_state: PlayState) {
        self.save_static_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_play_state(),
            &play_state,
        );
    }

    fn play_state(&self) -> Option<PlayState> {
        let (_, play_state) = self
            .current_blueprint()
            .latest_at_component_quiet::<PlayState>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_play_state().component,
            )?;

        Some(play_state)
    }

    fn set_loop_mode(&self, loop_mode: LoopMode) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_loop_mode(),
            &loop_mode,
        );
    }

    fn loop_mode(&self) -> Option<LoopMode> {
        let (_, loop_mode) = self
            .current_blueprint()
            .latest_at_component_quiet::<LoopMode>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_loop_mode().component,
            )?;

        Some(loop_mode)
    }

    fn set_time_selection(&self, time_range: AbsoluteTimeRange) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_time_selection(),
            &re_sdk_types::blueprint::components::AbsoluteTimeRange(
                re_sdk_types::datatypes::AbsoluteTimeRange {
                    min: time_range.min.as_i64().into(),
                    max: time_range.max.as_i64().into(),
                },
            ),
        );
    }

    fn time_selection(&self) -> Option<AbsoluteTimeRange> {
        let (_, time_range) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::AbsoluteTimeRange>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_time_selection().component,
        )?;

        Some(AbsoluteTimeRange::new(time_range.min, time_range.max))
    }

    fn clear_time_selection(&self) {
        self.clear_blueprint_component(
            time_panel_blueprint_entity_path(),
            TimePanelBlueprint::descriptor_time_selection(),
        );
    }
}
