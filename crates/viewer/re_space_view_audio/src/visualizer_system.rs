use re_data_store::{LatestAtQuery, TimeInt};
use re_query::{query_archetype, QueryError};
use re_types::{
    archetypes::{self, Audio},
    components,
    datatypes::TensorDimension,
};
use re_viewer_context::{
    external::re_log_types::{EntityPath, RowId},
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

// ---

#[derive(Debug, Clone)]
pub struct AudioEntry {
    /// Unique id for this audio.
    ///
    /// Used to avoid playing the same sound multiple times.
    pub row_id: RowId,

    pub entity_path: EntityPath,

    /// The timeline time for when the sounds was logged.
    ///
    /// This is the start of the sound on the timeline.
    ///
    /// `None` if timeless.
    pub data_time: Option<TimeInt>,

    /// PCM-encoded audio data
    pub data: components::TensorData,

    /// In Hz, e.g. 44100.
    ///
    /// This is commonly called the "sample rate" for historic reasons,
    /// but it is actually frames per second, where each audio frame consists of 2 samples for stereo.
    pub frame_rate: f32,

    /// Number of channels (1=mono, 2=stereo, etc).
    pub num_channels: Option<u64>,

    /// Number of frames (each frame is e.g. 2 samples for stereo audio).
    pub num_frames: Option<u64>,

    /// Length of sound in seconds.
    pub duration_sec: Option<f64>,
}

/// All audio data in the current view
#[derive(Default)]
pub struct AudioSystem {
    // Must be an `Option` because
    pub query: Option<LatestAtQuery>,

    // All the selected audio files.
    pub entries: Vec<AudioEntry>,
}

impl IdentifiedViewSystem for AudioSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Audio".into()
    }
}

impl VisualizerSystem for AudioSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Audio>()
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = ctx.entity_db.store();

        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        self.query = Some(timeline_query.clone());

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            // TODO(#3320): this match can go away once the issue is resolved
            match query_archetype::<archetypes::Audio>(
                store,
                &timeline_query,
                &data_result.entity_path,
            ) {
                Ok(arch_view) => {
                    let datas = arch_view.iter_required_component::<components::TensorData>()?;
                    let sample_rates =
                        arch_view.iter_optional_component::<components::AudioSampleRate>()?;

                    for (data, sample_rate) in itertools::izip!(datas, sample_rates) {
                        let num_samples = data.buffer.num_elements();
                        let num_channels = num_channels(&data.shape);
                        let frame_rate = sample_rate.map_or(44100.0, |r| r.0);

                        let mut num_frames = None;
                        let mut duration_sec = None;

                        if let (Some(num_samples), Some(num_channels)) = (num_samples, num_channels)
                        {
                            let frames = num_samples as u64 / num_channels;
                            num_frames = Some(frames);
                            duration_sec = Some(frames as f64 / frame_rate as f64);
                        }

                        self.entries.push(AudioEntry {
                            row_id: arch_view.primary_row_id(),
                            entity_path: data_result.entity_path.clone(), // TODO: instance path?
                            data,
                            data_time: arch_view.data_time(),
                            frame_rate,
                            num_channels,
                            num_frames,
                            duration_sec,
                        });
                    }
                }
                Err(QueryError::PrimaryNotFound(_)) => {}
                Err(err) => {
                    re_log::error_once!(
                        "Unexpected error querying {:?}: {err}",
                        &data_result.entity_path
                    );
                }
            };
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn num_channels(shape: &[TensorDimension]) -> Option<u64> {
    // Ignore leading and trailing unit-dimensions:
    let mut shape = shape.iter().map(|d| d.size).collect::<Vec<_>>();
    while shape.first() == Some(&1) {
        shape.remove(0);
    }
    while shape.last() == Some(&1) {
        shape.pop();
    }

    match shape.as_slice() {
        [] => Some(0),
        [_] => Some(1),
        [a, b] => {
            let [a, b] = [*a, *b];
            // Usually audio data is interleaved, so `b` is small:
            let max_channels = 24;
            if b <= max_channels {
                Some(b)
            } else if a <= max_channels {
                Some(a)
            } else {
                None
            }
        }
        _ => None,
    }
}
