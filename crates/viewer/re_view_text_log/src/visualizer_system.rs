use std::collections::HashMap;

use re_chunk_store::{AbsoluteTimeRange, ChunkTrackingMode, RangeQuery};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::TextLogRows;
use re_sdk_types::blueprint::encodings::ComponentSourceKind;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};
use re_viewport_blueprint::ViewProperty;

use crate::row_layout::{LevelCountCache, LevelFilter, RowLayout, RowOverrides};

/// Result of executing [`TextLogSystem`] for one frame: the row layout of the text log table.
///
/// No row data is materialized here: the layout is derived from chunk-level metadata of the
/// range query results (which are densified around the text component and pre-sorted on the
/// query timeline), and the view resolves only the rows that are actually on screen.
#[derive(Clone, Default)]
pub struct TextLogOutput {
    /// `None` if the visualizer didn't run at all, i.e. the view has no active instructions.
    pub layout: Option<RowLayout>,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct TextLogSystem;

impl IdentifiedViewSystem for TextLogSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "TextLog"
        )
    }
}

impl VisualizerSystem for TextLogSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Text>(
            &TextLog::descriptor_text(),
            &TextLog::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();

        // A text log row's level/color are read from that same row only (plus per-entity
        // blueprint overrides/defaults, see below), and other timelines' timestamps may be
        // shown as columns, so keep all of the chunks' columns around.
        //
        // Note that this means latest-at semantics do not apply to the level/color of a row
        // (which is desirable for this archetype: a log line should not inherit the previous
        // line's level).
        let query = RangeQuery::new(view_query.timeline, AbsoluteTimeRange::EVERYTHING)
            .keep_extra_timelines(true)
            .keep_extra_components(true);

        let component = TextLog::descriptor_text().component;
        let level_component = TextLog::descriptor_level().component;
        let color_component = TextLog::descriptor_color().component;

        let latest_at_query = ctx.current_query();
        let engine = ctx.recording_engine();

        let mut chunks = Vec::new();
        let mut overrides = HashMap::new();
        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let mut results = engine.cache().range(
                ChunkTrackingMode::Report,
                &query,
                &data_result.entity_path,
                [component],
            );

            if !results.missing_virtual.is_empty() {
                output.set_missing_chunks();
            }

            if let Some(entity_chunks) = results.components.remove(&component) {
                chunks.extend(entity_chunks);
            }

            // Blueprint overrides and view defaults for level/color are per-entity constants,
            // which is what keeps them compatible with the metadata-derived row layout.
            // The standard `Override > Store > Default` resolution decides where a component
            // comes from; only when it's the store do the rows' own values apply (per row,
            // straight from the chunks — see `RowLayout`).
            let resolved = re_view::latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_at_query,
                data_result,
                [level_component, color_component],
                Some(instruction),
            );

            let mut entity_overrides = RowOverrides::default();
            match resolved.component_source_kind_for(level_component) {
                Some(Ok(ComponentSourceKind::Override)) => {
                    entity_overrides.level_override = resolved
                        .get_mono::<TextLogLevel>(level_component)
                        .map(|lvl| lvl.as_str().to_owned());
                }
                Some(Ok(ComponentSourceKind::Default)) => {
                    entity_overrides.level_default = resolved
                        .get_mono::<TextLogLevel>(level_component)
                        .map(|lvl| lvl.as_str().to_owned());
                }
                _ => {}
            }
            match resolved.component_source_kind_for(color_component) {
                Some(Ok(ComponentSourceKind::Override)) => {
                    entity_overrides.color_override = resolved.get_mono::<Color>(color_component);
                }
                Some(Ok(ComponentSourceKind::Default)) => {
                    entity_overrides.color_default = resolved.get_mono::<Color>(color_component);
                }
                _ => {}
            }
            overrides.insert(data_result.entity_path.clone(), entity_overrides);
        }

        // An *explicit* level filter (i.e. one set in the blueprint, not the show-everything
        // fallback) changes which rows are shown; the row layout then comes from cached
        // per-chunk level counts instead of plain chunk row counts.
        let filter = ViewProperty::from_archetype::<TextLogRows>(ctx)
            .component_array::<TextLogLevel>(
                TextLogRows::descriptor_filter_by_log_level().component,
            )?
            .map(|levels| LevelFilter(levels.iter().map(|lvl| lvl.as_str().to_owned()).collect()));

        let layout = ctx
            .viewer_ctx
            .store_context
            .memoizer(|level_counts: &mut LevelCountCache| {
                RowLayout::build(
                    chunks,
                    &view_query.timeline,
                    component,
                    level_component,
                    color_component,
                    filter.as_ref(),
                    &overrides,
                    level_counts,
                )
            });

        Ok(output.with_visualizer_data(TextLogOutput {
            layout: Some(layout),
        }))
    }
}
