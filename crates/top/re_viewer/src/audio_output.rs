//! The viewer's shared audio output.

/// The one audio output of the viewer, owned by [`crate::AppState`].
///
/// Empty when built without the `audio` feature.
#[derive(Default)]
pub struct AudioOutput {
    /// Opens the output device on first use.
    #[cfg(feature = "audio")]
    pub player: re_audio::AudioPlayer,
}

#[cfg(debug_assertions)]
impl AudioOutput {
    /// Stage a short generated tone for playback.
    pub fn play_test_sound(&self) {
        cfg_select! {
            feature = "audio" => {
                // Replaying the test sound restarts it rather than layering a second copy.
                const TEST_SOUND_ID: re_audio::StreamId = re_audio::StreamId(0);
                self.player.play_once(TEST_SOUND_ID, re_audio::test_sound());
            }
            _ => {
                let _ = self;
            }
        }
    }
}

impl AudioOutput {
    /// Commit audio requested by this pass, or discard it with the rest of the pass.
    pub fn finish_ui_pass(&self, will_discard: bool) {
        cfg_select! {
            feature = "audio" => {
                if will_discard {
                    self.player.discard_requests();
                } else {
                    self.player.commit_requests();
                }
            }
            _ => {
                let _ = (self, will_discard);
            }
        }
    }
}
