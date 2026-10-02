use crate::audio_asset_cache::{AudioAssetCache, AudioLoadState};
use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::AssetAudio;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::components::{Blob, MediaType};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    ViewerDiagnostic, ViewerReportSeverity, VisualizerExecutionOutput, VisualizerInstructionReport,
    VisualizerQueryInfo, VisualizerReportContext, VisualizerSystem,
};

/// One audio asset visible in the view.
#[derive(Clone)]
pub struct AudioEntry {
    pub entity_path: EntityPath,

    /// When the asset was logged on the view's timeline, which is when playback starts.
    ///
    /// Never [`TimeInt::STATIC`]: static audio cannot be placed in time, so it gets no entry.
    pub data_time: TimeInt,

    pub media_type: Option<MediaType>,
    pub state: AudioLoadState,
}

#[derive(Default)]
pub struct AudioAssetVisualizer;

impl IdentifiedViewSystem for AudioAssetVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "AssetAudio"
        )
    }
}

impl VisualizerSystem for AudioAssetVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Blob>(
            &AssetAudio::descriptor_blob(),
            &AssetAudio::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);
        let blob_component = AssetAudio::descriptor_blob().component;
        let media_type_component = AssetAudio::descriptor_media_type().component;

        let output = VisualizerExecutionOutput::default();
        let mut entries = Vec::new();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let entity_path = &data_result.entity_path;

            let results = data_result.latest_at_with_blueprint_resolved_data::<AssetAudio>(
                ctx,
                &query,
                Some(instruction),
            );
            if results.any_missing_chunks() {
                output.set_missing_chunks();
            }

            let blob_chunk = match results.get_unit_chunk_with_source(blob_component, true) {
                Some((_, Ok(Some(blob_chunk)))) => blob_chunk,
                Some((_, Ok(None))) | None => continue,
                Some((_, Err(err))) => {
                    report_blob_error(&output, instruction.id, err.to_string(), None);
                    continue;
                }
            };
            let Some((data_time, row_id)) = blob_chunk.index(Some(&view_query.timeline)) else {
                continue;
            };
            let Some(blob) = blob_chunk
                .component_mono::<Blob>(blob_component)
                .and_then(Result::ok)
            else {
                continue;
            };

            if data_time == TimeInt::STATIC {
                report_blob_error(
                    &output,
                    instruction.id,
                    "Static audio cannot be played".to_owned(),
                    Some(
                        "Audio starts playing at the time it was logged. \
                         Log it on a timeline to play it."
                            .to_owned(),
                    ),
                );
                continue;
            }

            let media_type = results.get_mono::<MediaType>(media_type_component);

            let state = ctx
                .viewer_ctx
                .store_context
                .memoizer(|cache: &mut AudioAssetCache| {
                    cache.entry(
                        entity_path.to_string(),
                        row_id,
                        blob_component,
                        &blob,
                        media_type.as_ref(),
                    )
                });

            if let AudioLoadState::Failed(err) = &state {
                report_blob_error(
                    &output,
                    instruction.id,
                    "Failed to decode audio".to_owned(),
                    Some(err.to_string()),
                );
                continue;
            }

            entries.push(AudioEntry {
                entity_path: entity_path.clone(),
                data_time,
                media_type,
                state,
            });
        }

        Ok(output.with_visualizer_data(entries))
    }
}

fn report_blob_error(
    output: &VisualizerExecutionOutput,
    instruction_id: VisualizerInstructionId,
    summary: String,
    details: Option<String>,
) {
    output.report(
        instruction_id,
        VisualizerInstructionReport {
            diagnostic: ViewerDiagnostic {
                severity: ViewerReportSeverity::Error,
                summary,
                details,
            },
            context: VisualizerReportContext {
                component: Some(AssetAudio::descriptor_blob().component),
                extra: None,
            },
        },
    );
}
